"""Local guide video preparation. External tools never receive shell commands."""
from __future__ import annotations

import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time
import sys
import importlib.util
from uuid import uuid4

from .import_sources import youtube_url, ytdlp_path

MAX_PART_BYTES = 150_000_000
OVERLAP = 15
PRESET_VERSION = 2
FLAGS = getattr(subprocess, 'CREATE_NO_WINDOW', 0) | getattr(subprocess, 'BELOW_NORMAL_PRIORITY_CLASS', 0)


def ffmpeg_path():
    configured = os.environ.get('HOI4_FFMPEG') or shutil.which('ffmpeg')
    if configured:
        return configured
    try:
        from imageio_ffmpeg import get_ffmpeg_exe
        return get_ffmpeg_exe()
    except ImportError:
        raise ValueError('FFmpeg non trovato: installa le dipendenze video o configura HOI4_FFMPEG') from None


def command(args, timeout=60):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True,
                            encoding='utf8', errors='replace', timeout=timeout, creationflags=FLAGS)
    if result.returncode:
        raise ValueError('Strumento video: ' + result.stderr[-1200:])
    return result.stdout


def duration(ffmpeg, path):
    result = subprocess.run([str(ffmpeg), '-hide_banner', '-i', str(path)], capture_output=True,
                            text=True, encoding='utf8', errors='replace', timeout=25, creationflags=FLAGS)
    match = re.search(r'Duration: (\d+):(\d+):(\d+(?:\.\d+)?)', result.stderr)
    if not match:
        raise ValueError('Durata del video non leggibile')
    return int(match[1])*3600 + int(match[2])*60 + float(match[3])


def choose_encoder(ffmpeg):
    """An advertised encoder is not evidence that its driver/device works."""
    candidates = [
        ('NVIDIA NVENC', ['-c:v','hevc_nvenc','-preset','p6','-rc','vbr','-cq','22','-b:v','0','-g','15','-forced-idr','1']),
        ('Intel QSV', ['-c:v','hevc_qsv','-preset','medium','-global_quality','22','-g','15','-flags','+cgop']),
        ('AMD AMF', ['-c:v','hevc_amf','-quality','quality','-rc','cqp','-qp_i','22','-qp_p','22','-g','15']),
        ('CPU x265', ['-c:v','libx265','-preset','medium','-crf','22',
                      '-x265-params','pools=4:frame-threads=2:log-level=error:open-gop=0:keyint=15:min-keyint=15:scenecut=0']),
    ]
    failures = []
    for name, args in candidates:
        try:
            command([ffmpeg,'-v','error','-f','lavfi','-i','color=size=1280x720:rate=15',
                     '-frames:v','3',*args,'-f','null','-'], timeout=30)
            return name, args, failures
        except (ValueError, subprocess.TimeoutExpired):
            failures.append(name)
    raise ValueError('Nessun encoder H.265 utilizzabile')


def split_video(ffmpeg, master, folder, seconds, update=lambda **kw: None, limit=MAX_PART_BYTES):
    """Copy at forced one-second keyframes; verify every file against a hard cap.

    Adaptive retries handle variable bitrate. Nominal time ranges are saved for
    merging AI citations; adjacent ranges overlap by exactly fifteen seconds.
    """
    if seconds <= 0:
        raise ValueError('Video vuoto')
    result, start = [], 0
    size = master.stat().st_size
    while start < seconds:
        remaining = seconds-start
        length = remaining if size <= limit else min(remaining, max(OVERLAP+2, math.floor(seconds*limit/size*.92)))
        target = folder / f'parte-{len(result)+1:02d}.mp4'
        for attempt in range(24):
            command([ffmpeg,'-v','error','-nostdin','-y','-ss',str(start),'-i',str(master),
                     '-t',str(length),'-map','0:v:0','-map','0:a:0?', '-c','copy',
                     '-avoid_negative_ts','make_zero','-movflags','+faststart',str(target)], timeout=180)
            actual = target.stat().st_size
            if actual <= limit:
                if abs(duration(ffmpeg,target)-length)>.5:
                    raise ValueError('Taglio non allineato al minuto originale: encoder senza punti IDR adeguati')
                break
            reduced = math.floor(length * limit/actual *.9)
            if reduced <= OVERLAP or reduced >= length:
                raise ValueError('Impossibile rispettare 150 MB conservando la sovrapposizione e la qualità')
            length = reduced
        else:
            raise ValueError('Limite parti video non raggiunto')
        end = min(seconds, start+length)
        result.append({'name':target.name,'start_seconds':start,'end_seconds':end,'bytes':actual})
        update(stage='splitting', message=f'Parte {len(result)} verificata: {actual/1e6:.1f} MB')
        if end >= seconds:
            break
        next_start = end-OVERLAP
        if next_start <= start:
            raise ValueError('Intervallo video troppo breve')
        start = next_start
    return result


class VideoJobs:
    def __init__(self, root):
        self.root = Path(root)
        self.lock = threading.RLock()
        self.jobs = {}
        self.processes = {}
        self.closed = False

    def close(self):
        with self.lock:
            self.closed=True
            children=list(self.processes.values())
        for process in children:
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:process.kill();process.wait(timeout=3)

    def _own(self,key,process):
        with self.lock:
            if self.closed:
                process.terminate();process.wait(timeout=5)
                raise ValueError('Preparazione interrotta: programma chiuso')
            self.processes[key]=process

    def _download(self,key,args,log):
        process=subprocess.Popen(args,stdout=log,stderr=log,creationflags=FLAGS)
        self._own(key,process)
        try:
            if process.wait(timeout=3600):raise ValueError('Download YouTube non riuscito; controlla la disponibilità pubblica del video')
        except subprocess.TimeoutExpired:
            process.kill();process.wait();raise ValueError('Download oltre il limite di tempo') from None
        finally:
            with self.lock:self.processes.pop(key,None)

    def status(self, key):
        if not re.fullmatch('[0-9a-f]{32}', key or ''):
            raise ValueError('Identificatore video non valido')
        with self.lock:
            if key not in self.jobs:
                path=self.root/key/'manifest.json'
                if not path.is_file():raise ValueError('Preparazione video non trovata')
                manifest=json.loads(path.read_text(encoding='utf8'))
                if manifest.get('preset_version')!=PRESET_VERSION:
                    raise ValueError('Video creato con un preset precedente: preparalo di nuovo dal link')
                self.jobs[key]={**manifest,'id':key,'status':'completed','stage':'completed','message':'Video pronto',
                    'manifest_url':f'/api/import/video/file?id={key}&name=manifest.json',
                    'description_url':f'/api/import/video/file?id={key}&name=descrizione.txt'}
            return json.loads(json.dumps(self.jobs[key]))

    def start(self, url):
        canonical = youtube_url(url)
        with self.lock:
            if self.closed:raise ValueError('Preparazione video chiusa')
            for manifest_path in sorted(self.root.glob('*/manifest.json'),key=lambda p:p.stat().st_mtime_ns,reverse=True):
                try:
                    saved=json.loads(manifest_path.read_text(encoding='utf8'))
                    if (saved.get('preset_version')==PRESET_VERSION and saved.get('source_url')==canonical and saved.get('fps')==15
                            and saved.get('max_part_bytes')==MAX_PART_BYTES and saved.get('overlap_seconds')==OVERLAP
                            and saved.get('parts') and all(re.fullmatch(r'parte-\d+\.mp4',part.get('name',''))
                                and (manifest_path.parent/part['name']).is_file()
                                and (manifest_path.parent/part['name']).stat().st_size==part['bytes']<=MAX_PART_BYTES for part in saved['parts'])):
                        return self.status(manifest_path.parent.name)
                except (ValueError,OSError,KeyError,TypeError):continue
            if any(job['status']=='running' for job in self.jobs.values()):
                raise ValueError('Una preparazione video è già in corso')
            key = uuid4().hex
            self.jobs[key] = {'id':key,'url':canonical,'status':'running','stage':'queued',
                              'message':'Preparazione avviata','parts':[]}
        threading.Thread(target=self._run, args=(key,canonical), daemon=True).start()
        return self.status(key)

    def update(self, key, **values):
        with self.lock:
            self.jobs[key].update(values)

    def _encode(self, key, ffmpeg, source, target, args):
        folder=target.parent
        progress=folder/'encoding-progress.txt'
        with (folder/'encoding.log').open('w',encoding='utf8') as log:
            process=subprocess.Popen([str(ffmpeg),'-hide_banner','-nostdin','-nostats','-y','-threads','4',
                '-i',str(source),'-map','0:v:0','-map','0:a:0?',
                '-vf',"scale=-2:'min(1080,ih)',fps=15",*args,'-pix_fmt','yuv420p','-tag:v','hvc1',
                '-force_key_frames','expr:gte(t,n_forced*1)','-c:a','aac','-ac','1','-b:a','64k',
                '-movflags','+faststart','-map_metadata','-1','-progress',str(progress),str(target)],
                stdout=subprocess.DEVNULL,stderr=log,creationflags=FLAGS)
            self._own(key,process)
            deadline=time.monotonic()+6*3600
            while process.poll() is None:
                if time.monotonic()>deadline:
                    process.kill();process.wait();raise ValueError('Codifica video oltre il limite di tempo')
                try:
                    times=re.findall(r'out_time_us=(\d+)',progress.read_text(encoding='utf8'))
                    if times:self.update(key,encoded_seconds=round(int(times[-1])/1e6,1))
                except OSError:pass
                time.sleep(1)
            if process.returncode:
                raise ValueError('Codifica video non riuscita: '+(folder/'encoding.log').read_text(encoding='utf8')[-1200:])
            with self.lock:self.processes.pop(key,None)

    def _run(self, key, url):
        folder=self.root/key
        try:
            folder.mkdir(parents=True,exist_ok=False)
            ffmpeg=ffmpeg_path();ytdlp=ytdlp_path()
            from .guide_frames import GuideFrames
            frames=GuideFrames(self.root.parent/'guide-frames')
            try:cached=frames.source(url)
            except ValueError:cached=None
            module_available=importlib.util.find_spec('yt_dlp') is not None
            if not ytdlp and not module_available and cached is None:raise ValueError('yt-dlp non trovato: installa le dipendenze video o configura HOI4_YTDLP')
            launcher=[ytdlp] if os.environ.get('HOI4_YTDLP') or not module_available else [sys.executable,'-m','yt_dlp']
            base=[*launcher,'--ignore-config','--no-plugin-dirs','--no-playlist','--socket-timeout','20','--retries','2']
            self.update(key,stage='metadata',message='Lettura delle informazioni YouTube')
            metadata={'title':url,'duration':duration(ffmpeg,cached),'description':'',**frames.metadata(url)} if cached else json.loads(command([*base,'--skip-download','--dump-single-json','--',url],timeout=120))
            seconds=metadata.get('duration')
            if not isinstance(seconds,(int,float)) or not 0<seconds<=14400 or metadata.get('is_live'):
                raise ValueError('Usa un video registrato di durata inferiore a quattro ore')
            self.update(key,title=metadata.get('title','Video guida'),duration_seconds=seconds,
                        stage='downloading',message='Download del sorgente fino a 1080p, con audio originale inglese quando disponibile')
            (folder/'descrizione.txt').write_text(metadata.get('description',''),encoding='utf8')
            # A fixed output directory and generated basename prevent media metadata
            # from selecting filesystem paths. No login/cookies/config/plugin code.
            if cached:
                sources=[cached]
                self.update(key,message='Riutilizzo del sorgente locale: nessun nuovo download',cache_reused=True)
            else:
                with (folder/'download.log').open('w',encoding='utf8') as log:
                    self._download(key,[*base,'--no-progress','--ffmpeg-location',str(ffmpeg),
                        '-f','bv[height<=1080]+ba[language^=en]/bv[height<=1080]+ba/b[height<=1080]',
                        '--merge-output-format','mkv','-o',str(folder/'source.%(ext)s'),'--',url],
                        log)
                sources=[p for p in folder.glob('source.*') if p.suffix in ('.mkv','.mp4','.webm')]
            if len(sources)!=1:raise ValueError('Sorgente video scaricato non identificabile')
            frames.register(url,sources[0],metadata)
            name,args,unavailable=choose_encoder(ffmpeg)
            self.update(key,encoder=name,unavailable_encoders=unavailable,stage='encoding',
                        message=f'Codifica H.265 a 15 FPS · {name}',quality_mode='CRF 22' if name=='CPU x265' else 'Qualità hardware 22 (non equivalente numericamente a CRF)')
            master=folder/'master.mp4'
            try:self._encode(key,ffmpeg,sources[0],master,args)
            except ValueError:
                if name=='CPU x265':raise
                args=['-c:v','libx265','-preset','medium','-crf','22','-x265-params','pools=4:frame-threads=2:log-level=error:open-gop=0:keyint=15:min-keyint=15:scenecut=0']
                self.update(key,encoder='CPU x265',quality_mode='CRF 22',message='Encoder hardware non riuscito sul sorgente: riprovo con CPU x265')
                self._encode(key,ffmpeg,sources[0],master,args)
            real_seconds=duration(ffmpeg,master)
            try:parts=split_video(ffmpeg,master,folder,real_seconds,lambda **kw:self.update(key,**kw))
            except ValueError:
                if self.status(key)['encoder']=='CPU x265':raise
                self.update(key,encoder='CPU x265',quality_mode='CRF 22',stage='encoding',message='Riallineamento dei tagli con CPU x265')
                self._encode(key,ffmpeg,sources[0],master,['-c:v','libx265','-preset','medium','-crf','22','-x265-params','pools=4:frame-threads=2:log-level=error:open-gop=0:keyint=15:min-keyint=15:scenecut=0'])
                real_seconds=duration(ffmpeg,master)
                parts=split_video(ffmpeg,master,folder,real_seconds,lambda **kw:self.update(key,**kw))
            for part in parts:
                part['url']=f'/api/import/video/file?id={key}&name={part["name"]}'
            manifest={'preset_version':PRESET_VERSION,'source_url':url,'title':metadata.get('title',url),'fps':15,'max_part_bytes':MAX_PART_BYTES,'overlap_seconds':OVERLAP,
                      'duration_seconds':real_seconds,'parts':parts,'encoder':self.status(key)['encoder'],
                      'quality_mode':self.status(key)['quality_mode']}
            (folder/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
            self.update(key,status='completed',stage='completed',message='Video pronto: ogni parte è stata verificata sotto 150 MB',parts=parts,
                        manifest_url=f'/api/import/video/file?id={key}&name=manifest.json',
                        description_url=f'/api/import/video/file?id={key}&name=descrizione.txt')
        except Exception as exc:
            self.update(key,status='failed',stage='failed',message=str(exc)[:1600])
        finally:
            with self.lock:self.processes.pop(key,None)

    def file(self,key,name):
        job=self.status(key)
        allowed={p['name'] for p in job['parts']} | {'manifest.json','descrizione.txt'}
        if job['status']!='completed' or name not in allowed:
            raise ValueError('File video non disponibile')
        return self.root/key/name
