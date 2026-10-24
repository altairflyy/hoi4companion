"""Frames are local source evidence, never screenshots of the current game."""
import hashlib
import json
from pathlib import Path
import threading

from .guide_media import command, duration, ffmpeg_path
from .import_sources import youtube_url


class GuideFrames:
    def __init__(self,root):
        self.root=Path(root)
        self.lock=threading.RLock()

    def register(self,url,path,metadata=None):
        canonical=youtube_url(url)
        target=Path(path).resolve()
        if not target.is_file():raise ValueError('Video locale non trovato')
        self.root.mkdir(parents=True,exist_ok=True)
        registry=self.root/'sources.json'
        data=json.loads(registry.read_text(encoding='utf8')) if registry.exists() else {}
        data[canonical]={'path':str(target),**{k:metadata[k] for k in ('title','duration','description') if isinstance(metadata,dict) and k in metadata}}
        temporary=registry.with_suffix('.tmp')
        temporary.write_text(json.dumps(data,indent=2),encoding='utf8');temporary.replace(registry)

    def source(self,url):
        canonical=youtube_url(url)
        registry=self.root/'sources.json'
        data=json.loads(registry.read_text(encoding='utf8')) if registry.exists() else {}
        item=data.get(canonical,'')
        path=Path(item.get('path','') if isinstance(item,dict) else item)
        if not path.is_file():raise ValueError('Video sorgente non disponibile: preparalo nella pagina Importa guida')
        return path

    def metadata(self,url):
        registry=self.root/'sources.json'
        data=json.loads(registry.read_text(encoding='utf8')) if registry.exists() else {}
        item=data.get(youtube_url(url),{})
        return {k:v for k,v in item.items() if k!='path'} if isinstance(item,dict) else {}

    def frame(self,url,seconds):
        if type(seconds) not in (int,float) or not 0<=seconds<14400:raise ValueError('Minutaggio non valido')
        with self.lock:
            source=self.source(url)
            stamp=f'{source}:{source.stat().st_mtime_ns}:{seconds}'
            target=self.root/(hashlib.sha256(stamp.encode()).hexdigest()+'.jpg')
            if not target.exists():
                ffmpeg=ffmpeg_path()
                if seconds>=duration(ffmpeg,source):raise ValueError('Fotogramma oltre la durata del video')
                command([ffmpeg,'-v','error','-nostdin','-ss',str(seconds),'-i',str(source),
                         '-frames:v','1','-q:v','2','-y',str(target)],timeout=40)
            return target


def frame_reference(guide,step):
    params=step['action'].get('params',{})
    visual=[e for e in step.get('evidence',[]) if e.get('basis')=='visual' and type(e.get('timestamp_seconds')) in (int,float)]
    timed=visual or [e for e in step.get('evidence',[]) if type(e.get('timestamp_seconds')) in (int,float)]
    seconds=params.get('source_seconds',timed[0]['timestamp_seconds'] if timed else None)
    if type(seconds) not in (int,float) or not 0<=seconds<14400:return None
    sources={s['id']:s for s in guide.get('sources',[])}
    linked=[sources[e['source_id']] for e in timed if e.get('source_id') in sources]
    youtube=[s for s in guide.get('sources',[]) if s.get('type')=='youtube']
    linked_urls=list(dict.fromkeys(s['url'] for s in linked if s.get('url') and s.get('type')=='youtube'))
    urls=linked_urls or list(dict.fromkeys(s['url'] for s in youtube if s.get('url')))
    if len(urls)!=1:return None
    return {'url':youtube_url(urls[0]),'seconds':seconds}
