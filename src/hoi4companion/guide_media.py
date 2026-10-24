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
from .video_cache import MasterCache, digest

MAX_PART_BYTES = 150_000_000
OVERLAP = 10
SEGMENT_SECONDS = 300
PRESET_VERSION = 4
KEYFRAME_SECONDS = 10
CPU_ARGS = ['-c:v','libx265','-preset','medium','-crf','22',
            '-x265-params','pools=4:frame-threads=2:log-level=error:open-gop=0:keyint=150:min-keyint=150:scenecut=0']
ENCODING_PROFILE = {'version':1,'height_limit':1080,'fps':15,'codec':'hevc','quality':22,
                    'audio':'aac-mono-64k','keyframe_seconds':KEYFRAME_SECONDS,'encoder_selection':'sample-size-hardware-within-25-percent'}
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


def command(args, timeout=60, check=True):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True,
                            encoding='utf8', errors='replace', timeout=timeout, creationflags=FLAGS)
    if check and result.returncode:
        raise ValueError('Strumento video: ' + result.stderr[-1200:])
    return result.stdout if check else result.stderr+result.stdout


def media_info(ffmpeg, path, runner=None):
    output=(runner or command)([str(ffmpeg),'-hide_banner','-i',str(path)],timeout=25,check=False)
    match = re.search(r'Duration: (\d+):(\d+):(\d+(?:\.\d+)?)',output)
    if not match:
        raise ValueError('Durata del video non leggibile')
    video=next((line for line in output.splitlines() if 'Video:' in line),'')
    dimensions=re.search(r'\b(\d{2,5})x(\d{2,5})\b',video)
    return {'duration_seconds':int(match[1])*3600+int(match[2])*60+float(match[3]),
            'width':int(dimensions[1]) if dimensions else None,'height':int(dimensions[2]) if dimensions else None}


def duration(ffmpeg,path,runner=None):
    return media_info(ffmpeg,path,runner)['duration_seconds']


def choose_encoder(ffmpeg, runner=None):
    """An advertised encoder is not evidence that its driver/device works."""
    candidates = [
        ('NVIDIA NVENC', ['-c:v','hevc_nvenc','-preset','p6','-rc','vbr','-cq','22','-b:v','0','-g','150','-bf','3','-forced-idr','1']),
        ('Intel QSV', ['-c:v','hevc_qsv','-preset','medium','-global_quality','22','-g','150','-flags','+cgop']),
        ('AMD AMF', ['-c:v','hevc_amf','-quality','quality','-rc','cqp','-qp_i','22','-qp_p','22','-g','150']),
        ('CPU x265', CPU_ARGS),
    ]
    failures = []
    for name, args in candidates:
        try:
            (runner or command)([ffmpeg,'-v','error','-f','lavfi','-i','color=size=1280x720:rate=15',
                     '-frames:v','3',*args,'-f','null','-'], timeout=30)
            return name, args, failures
        except (ValueError, subprocess.TimeoutExpired):
            failures.append(name)
    raise ValueError('Nessun encoder H.265 utilizzabile')


def split_video(ffmpeg, master, folder, seconds, update=lambda **kw: None, limit=MAX_PART_BYTES, runner=None):
    """Copy at ten-second keyframes; verify every file against a hard cap.

    Adaptive retries handle variable bitrate. Nominal time ranges are saved for
    merging AI citations; parts are at most five minutes, with ten-second overlaps.
    """
    if seconds <= 0:
        raise ValueError('Video vuoto')
    result, start = [], 0
    size = master.stat().st_size
    while start < seconds:
        remaining = seconds-start
        length = min(remaining, SEGMENT_SECONDS)
        if size > limit:
            length = min(length, max(OVERLAP+KEYFRAME_SECONDS, math.floor(seconds*limit/size*.92/KEYFRAME_SECONDS)*KEYFRAME_SECONDS))
        target = folder / f'parte-{len(result)+1:02d}.mp4'
        for attempt in range(24):
            (runner or command)([ffmpeg,'-v','error','-nostdin','-y','-ss',str(start),'-i',str(master),
                     '-t',str(length),'-map','0:v:0','-map','0:a:0?', '-c','copy',
                     '-avoid_negative_ts','make_zero','-movflags','+faststart',str(target)], timeout=180)
            actual = target.stat().st_size
            if actual <= limit:
                actual_duration=duration(ffmpeg,target,runner) if runner else duration(ffmpeg,target)
                if abs(actual_duration-length)>.5:
                    raise ValueError('Taglio non allineato al minuto originale: encoder senza punti IDR adeguati')
                break
            reduced = math.floor(length * limit/actual *.9/KEYFRAME_SECONDS)*KEYFRAME_SECONDS
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
        self.cache=MasterCache(self.root/'_masters',ENCODING_PROFILE)

    @staticmethod
    def _stop(process):
        if process.poll() is not None:return
        if os.name=='nt':
            # yt-dlp may own a merger process. Stop this owned process tree only.
            try:subprocess.run(['taskkill','/PID',str(process.pid),'/T','/F'],capture_output=True,timeout=8,creationflags=FLAGS)
            except (OSError,subprocess.TimeoutExpired):pass
        if process.poll() is None:process.kill()
        process.wait(timeout=8)

    def _command(self,key,args,timeout=60,check=True):
        if self.closed:raise ValueError('Preparazione interrotta: programma chiuso')
        process=subprocess.Popen(list(map(str,args)),stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                 text=True,encoding='utf8',errors='replace',creationflags=FLAGS)
        self._own(key,process)
        try:
            try:out,err=process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired:
                self._stop(process);process.communicate();raise ValueError('Strumento video oltre il limite di tempo') from None
            if self.closed:raise ValueError('Preparazione interrotta: programma chiuso')
            if check and process.returncode:raise ValueError('Strumento video: '+err[-1200:])
            return out if check else err+out
        finally:
            with self.lock:self.processes.pop(key,None)

    def close(self):
        with self.lock:
            self.closed=True
            children=list(self.processes.values())
        for process in children:
            self._stop(process)

    def _own(self,key,process):
        with self.lock:
            if self.closed:
                self._stop(process)
                raise ValueError('Preparazione interrotta: programma chiuso')
            self.processes[key]=process

    def _download(self,key,args,log):
        process=subprocess.Popen(args,stdout=log,stderr=log,creationflags=FLAGS)
        self._own(key,process)
        try:
            if process.wait(timeout=3600):raise ValueError('Download YouTube non riuscito; controlla la disponibilità pubblica del video')
        except subprocess.TimeoutExpired:
            self._stop(process);raise ValueError('Download oltre il limite di tempo') from None
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

    def start(self, url, refresh_source=False):
        if type(refresh_source) is not bool:raise ValueError('Opzione sorgente non valida')
        canonical = youtube_url(url)
        source_hash=None
        if not refresh_source:
            from .guide_frames import GuideFrames
            try:source_hash=digest(GuideFrames(self.root.parent/'guide-frames').source(canonical))
            except ValueError:pass
        with self.lock:
            if self.closed:raise ValueError('Preparazione video chiusa')
            for manifest_path in sorted(self.root.glob('*/manifest.json'),key=lambda p:p.stat().st_mtime_ns,reverse=True):
                try:
                    saved=json.loads(manifest_path.read_text(encoding='utf8'))
                    if (not refresh_source and saved.get('preset_version')==PRESET_VERSION and saved.get('source_url')==canonical and saved.get('fps')==15
                            and saved.get('encoding_profile')==ENCODING_PROFILE
                            and (source_hash is None or saved.get('source_sha256')==source_hash)
                            and saved.get('max_part_bytes')==MAX_PART_BYTES and saved.get('overlap_seconds')==OVERLAP
                            and saved.get('segment_seconds')==SEGMENT_SECONDS
                            and saved.get('parts') and all(re.fullmatch(r'parte-\d+\.mp4',part.get('name',''))
                                and (manifest_path.parent/part['name']).is_file()
                                and (manifest_path.parent/part['name']).stat().st_size==part['bytes']<=MAX_PART_BYTES
                                and (not part.get('sha256') or digest(manifest_path.parent/part['name'])==part['sha256']) for part in saved['parts'])):
                        return self.status(manifest_path.parent.name)
                except (ValueError,OSError,KeyError,TypeError):continue
            if any(job['status']=='running' for job in self.jobs.values()):
                raise ValueError('Una preparazione video è già in corso')
            key = uuid4().hex
            self.jobs[key] = {'id':key,'url':canonical,'status':'running','stage':'queued',
                              'message':'Preparazione avviata','parts':[],'refresh_source':refresh_source}
        threading.Thread(target=self._run, args=(key,canonical), daemon=True).start()
        return self.status(key)

    def update(self, key, **values):
        with self.lock:
            self.jobs[key].update(values)

    def _encode(self, key, ffmpeg, source, target, args):
        folder=target.parent
        progress=folder/'encoding-progress.txt'
        progress.write_text('',encoding='utf8')
        with (folder/'encoding.log').open('w',encoding='utf8') as log:
            process=subprocess.Popen([str(ffmpeg),'-hide_banner','-nostdin','-nostats','-y','-threads','4',
                '-i',str(source),'-map','0:v:0','-map','0:a:0?',
                '-vf',"scale=-2:'min(1080,ih)',fps=15",*args,'-pix_fmt','yuv420p','-tag:v','hvc1',
                '-force_key_frames',f'expr:gte(t,n_forced*{KEYFRAME_SECONDS})','-c:a','aac','-ac','1','-b:a','64k',
                '-movflags','+faststart','-map_metadata','-1','-progress',str(progress),str(target)],
                stdout=subprocess.DEVNULL,stderr=log,creationflags=FLAGS)
            self._own(key,process)
            deadline=time.monotonic()+6*3600
            offset=0
            while process.poll() is None:
                if time.monotonic()>deadline:
                    self._stop(process);raise ValueError('Codifica video oltre il limite di tempo')
                try:
                    with progress.open(encoding='utf8') as stream:
                        stream.seek(offset);chunk=stream.read();offset=stream.tell()
                    times=re.findall(r'out_time_us=(\d+)',chunk)
                    if times:self.update(key,encoded_seconds=round(int(times[-1])/1e6,1))
                except OSError:pass
                time.sleep(1)
            if process.returncode:
                raise ValueError('Codifica video non riuscita: '+(folder/'encoding.log').read_text(encoding='utf8')[-1200:])
            with self.lock:self.processes.pop(key,None)

    def _select_encoder(self,key,ffmpeg,source,info,folder):
        runner=lambda args,**kw:self._command(key,args,**kw)
        name,args,unavailable=choose_encoder(ffmpeg,runner)
        if name=='CPU x265':return name,args,unavailable,[]
        measurements=[]
        for label,encoder in [(name,args),('CPU x265',CPU_ARGS)]:
            size=0;started=time.monotonic()
            try:
                for index,fraction in enumerate((.2,.6)):
                    target=folder/f'calibration-{index}.mp4'
                    runner([ffmpeg,'-v','error','-nostdin','-y','-threads','4','-ss',str(info['duration_seconds']*fraction),
                        '-i',str(source),'-t',str(min(10,info['duration_seconds']*(1-fraction))),
                        '-map','0:v:0','-an','-vf',"scale=-2:'min(1080,ih)',fps=15",*encoder,
                        '-pix_fmt','yuv420p','-force_key_frames',f'expr:gte(t,n_forced*{KEYFRAME_SECONDS})',str(target)],timeout=120)
                    size+=target.stat().st_size;target.unlink()
                measurements.append({'encoder':label,'bytes':size,'seconds':round(time.monotonic()-started,2)})
            except ValueError:
                if self.closed:raise
                unavailable.append(label)
            finally:
                for path in folder.glob('calibration-*.mp4'):
                    if path.is_file() and not path.is_symlink():path.unlink()
        if not measurements:raise ValueError('Nessun encoder ha completato i campioni reali')
        smallest=min(row['bytes'] for row in measurements)
        selected=next((row['encoder'] for row in measurements if row['encoder']==name and row['bytes']<=smallest*1.25),
                      min(measurements,key=lambda row:row['bytes'])['encoder'])
        return selected,args if selected==name else CPU_ARGS,unavailable,measurements

    def cleanup(self,url,keep,protected=()):
        """Delete only generated files in superseded same-URL job directories."""
        removed=0;root=self.root.resolve();protected={Path(p).resolve() for p in protected}
        generated=re.compile(r'(?:parte-\d+\.mp4|master\.mp4|source\.(?:mp4|mkv|webm)|encoding-progress\.txt|encoding\.log|download\.log|manifest\.json|descrizione\.txt|job\.json|calibration-\d+\.mp4)')
        for folder in self.root.iterdir():
            if folder.name==keep or not re.fullmatch('[0-9a-f]{32}',folder.name) or folder.is_symlink() or folder.resolve().parent!=root:continue
            with self.lock:
                if self.jobs.get(folder.name,{}).get('status')=='running':continue
            try:
                record=folder/'manifest.json'
                if not record.is_file():record=folder/'job.json'
                if json.loads(record.read_text(encoding='utf8')).get('source_url')!=url:continue
                for path in folder.iterdir():
                    if path.is_symlink() or not path.is_file() or path.resolve() in protected or not generated.fullmatch(path.name):continue
                    removed+=path.stat().st_size;path.unlink()
                if any(p.parent==folder.resolve() for p in protected):
                    (folder/'job.json').write_text(json.dumps({'source_url':url,'source_only':True}),encoding='utf8')
                if not any(folder.iterdir()):folder.rmdir()
                with self.lock:self.jobs.pop(folder.name,None)
            except (OSError,ValueError,KeyError):continue
        return removed

    def _run(self, key, url):
        folder=self.root/key
        try:
            folder.mkdir(parents=True,exist_ok=False)
            ffmpeg=ffmpeg_path();ytdlp=ytdlp_path()
            runner=lambda args,**kw:self._command(key,args,**kw)
            (folder/'job.json').write_text(json.dumps({'source_url':url}),encoding='utf8')
            from .guide_frames import GuideFrames
            frames=GuideFrames(self.root.parent/'guide-frames')
            try:cached=None if self.status(key).get('refresh_source') else frames.source(url)
            except ValueError:cached=None
            module_available=importlib.util.find_spec('yt_dlp') is not None
            if not ytdlp and not module_available and cached is None:raise ValueError('yt-dlp non trovato: installa le dipendenze video o configura HOI4_YTDLP')
            launcher=[ytdlp] if os.environ.get('HOI4_YTDLP') or not module_available else [sys.executable,'-m','yt_dlp']
            base=[*launcher,'--ignore-config','--no-plugin-dirs','--no-playlist','--socket-timeout','20','--retries','2']
            self.update(key,stage='metadata',message='Lettura delle informazioni YouTube')
            metadata={'title':url,'description':'',**frames.metadata(url),'duration':duration(ffmpeg,cached,runner)} if cached else json.loads(runner([*base,'--skip-download','--dump-single-json','--',url],timeout=120))
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
            info=media_info(ffmpeg,sources[0],runner)
            self.update(key,source_width=info['width'],source_height=info['height'])
            cache_key,identity=self.cache.identity(sources[0]);hit=self.cache.load(cache_key)
            if hit:
                master,details=hit;real_seconds=details['duration_seconds']
                self.update(key,encoder=details['encoder'],quality_mode=details['quality_mode'],master_reused=True,
                            message='Master compatibile verificato: salto la ricodifica',encoder_measurements=details.get('encoder_measurements',[]))
            else:
                self.update(key,stage='calibrating',message='Confronto GPU/CPU su campioni reali del video')
                name,args,unavailable,measurements=self._select_encoder(key,ffmpeg,sources[0],info,folder)
                quality='CRF 22' if name=='CPU x265' else 'Qualità hardware 22 (non equivalente numericamente a CRF)'
                self.update(key,encoder=name,unavailable_encoders=unavailable,stage='encoding',
                            message=f'Codifica H.265 a 15 FPS · {name}',quality_mode=quality,encoder_measurements=measurements)
                temporary=folder/'master.mp4'
                try:self._encode(key,ffmpeg,sources[0],temporary,args)
                except ValueError:
                    if name=='CPU x265' or self.closed:raise
                    self.update(key,encoder='CPU x265',quality_mode='CRF 22',message='Encoder hardware non riuscito sul sorgente: riprovo con CPU x265')
                    self._encode(key,ffmpeg,sources[0],temporary,CPU_ARGS)
                real_seconds=duration(ffmpeg,temporary,runner)
                if abs(real_seconds-info['duration_seconds'])>.5:raise ValueError('Durata della codifica incoerente con il sorgente')
                if self.cache.identity(sources[0])[0]!=cache_key:raise ValueError('Sorgente modificato durante la codifica')
                details={'source_url':url,'duration_seconds':real_seconds,'encoder':self.status(key)['encoder'],
                         'quality_mode':self.status(key)['quality_mode'],'encoder_measurements':measurements}
                master,details=self.cache.save(cache_key,identity,temporary,details)
            try:parts=split_video(ffmpeg,master,folder,real_seconds,lambda **kw:self.update(key,**kw),runner=runner)
            except ValueError:
                if self.status(key)['encoder']=='CPU x265' or self.closed:raise
                self.update(key,encoder='CPU x265',quality_mode='CRF 22',stage='encoding',message='Riallineamento dei tagli con CPU x265')
                temporary=folder/'master.mp4';self._encode(key,ffmpeg,sources[0],temporary,CPU_ARGS)
                real_seconds=duration(ffmpeg,temporary,runner)
                details.update(encoder='CPU x265',quality_mode='CRF 22',duration_seconds=real_seconds)
                master,details=self.cache.save(cache_key,identity,temporary,details)
                parts=split_video(ffmpeg,master,folder,real_seconds,lambda **kw:self.update(key,**kw),runner=runner)
            for part in parts:
                part['url']=f'/api/import/video/file?id={key}&name={part["name"]}'
                part['sha256']=digest(folder/part['name'])
            manifest={'preset_version':PRESET_VERSION,'source_url':url,'title':metadata.get('title',url),'fps':15,'max_part_bytes':MAX_PART_BYTES,'overlap_seconds':OVERLAP,'segment_seconds':SEGMENT_SECONDS,
                      'duration_seconds':real_seconds,'parts':parts,'encoder':self.status(key)['encoder'],
                      'quality_mode':self.status(key)['quality_mode'],'source_width':info['width'],'source_height':info['height'],
                      'source_sha256':identity['source_sha256'],'master_key':cache_key,'encoding_profile':ENCODING_PROFILE,
                      'encoder_measurements':details.get('encoder_measurements',[])}
            (folder/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
            self.update(key,status='completed',stage='completed',message='Video pronto: ogni parte è stata verificata sotto 150 MB',parts=parts,
                        manifest_url=f'/api/import/video/file?id={key}&name=manifest.json',
                        description_url=f'/api/import/video/file?id={key}&name=descrizione.txt',
                        source_width=info['width'],source_height=info['height'])
            try:
                registry=json.loads((frames.root/'sources.json').read_text(encoding='utf8'))
                protected=[item['path'] if isinstance(item,dict) else item for item in registry.values()]
                freed=self.cleanup(url,key,protected)+self.cache.cleanup(url,cache_key)
                self.update(key,cache_freed_bytes=freed)
            except (OSError,ValueError,KeyError):self.update(key,cache_warning='Video pronto; pulizia della cache non completata')
        except Exception as exc:
            self.update(key,status='failed',stage='failed',message=str(exc)[:1600])
        finally:
            with self.lock:self.processes.pop(key,None)
            temporary=folder/'master.mp4'
            if temporary.is_file() and not temporary.is_symlink():temporary.unlink()

    def file(self,key,name):
        job=self.status(key)
        allowed={p['name'] for p in job['parts']} | {'manifest.json','descrizione.txt'}
        if job['status']!='completed' or name not in allowed:
            raise ValueError('File video non disponibile')
        return self.root/key/name
