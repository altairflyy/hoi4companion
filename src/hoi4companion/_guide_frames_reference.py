"""Frames are local source evidence, never screenshots of the current game."""
import hashlib
import json
import math
from pathlib import Path
import subprocess
import threading
from uuid import uuid4

from .guide_media import command, duration, ffmpeg_path
from .import_sources import youtube_url

# Video jobs and the HTTP API own distinct GuideFrames objects for this registry.
# Serialize their read/modify/write transactions, not long-running encoders.
_registry_lock=threading.RLock()


def _valid_frame(path):
    from PIL import Image
    try:
        with Image.open(path) as image:
            if image.format!='JPEG':return False
            image.load()
        return True
    except (OSError,ValueError):return False


class GuideFrames:
    def __init__(self,root):
        self.root=Path(root)
        self.lock=threading.RLock()
        self.clip_lock=threading.RLock()

    def register(self,url,path,metadata=None):
        canonical=youtube_url(url)
        target=Path(path).resolve()
        if not target.is_file():raise ValueError('Video locale non trovato')
        self.root.mkdir(parents=True,exist_ok=True)
        registry=self.root/'sources.json'
        with _registry_lock:
            data=json.loads(registry.read_text(encoding='utf8')) if registry.exists() else {}
            data[canonical]={'path':str(target),**{k:metadata[k] for k in ('title','duration','description') if isinstance(metadata,dict) and k in metadata}}
            temporary=registry.with_name('sources-'+uuid4().hex+'.tmp')
            try:
                temporary.write_text(json.dumps(data,indent=2),encoding='utf8');temporary.replace(registry)
            finally:temporary.unlink(missing_ok=True)

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
            if not _valid_frame(target):
                ffmpeg=ffmpeg_path()
                if seconds>=duration(ffmpeg,source):raise ValueError('Fotogramma oltre la durata del video')
                temporary=self.root/('frame-tmp-'+uuid4().hex+'.jpg')
                try:
                    try:
                        command([ffmpeg,'-v','error','-nostdin','-ss',str(seconds),'-i',str(source),
                                 '-frames:v','1','-q:v','2','-y',str(temporary)],timeout=40)
                    except subprocess.TimeoutExpired as exc:
                        raise ValueError('Fotogramma non disponibile: tempo massimo di elaborazione superato') from exc
                    if not _valid_frame(temporary):raise ValueError('Fotogramma generato non valido')
                    temporary.replace(target)
                finally:temporary.unlink(missing_ok=True)
            return target

    def clip(self,url,seconds):
        """Generate bounded contextual video from a registered local source."""
        if type(seconds) not in (int,float) or not math.isfinite(seconds) or not 0<=seconds<14400:
            raise ValueError('Minutaggio non valido')
        with self.clip_lock:
            source=self.source(url)
            stamp=f'clip-v1:{source}:{source.stat().st_size}:{source.stat().st_mtime_ns}:{seconds}'
            target=self.root/('clip-'+hashlib.sha256(stamp.encode()).hexdigest()+'.mp4')
            if target.is_file():return target
            ffmpeg=ffmpeg_path();total=duration(ffmpeg,source)
            if seconds>=total:raise ValueError('Fotogramma oltre la durata del video')
            start=max(0,seconds-5);end=min(total,seconds+10)
            temporary=self.root/('clip-tmp-'+uuid4().hex+'.mp4')
            try:
                try:
                    command([ffmpeg,'-v','error','-nostdin','-ss',str(start),'-i',str(source),
                             '-t',str(end-start),'-map','0:v:0','-map','0:a:0?',
                             '-c:v','libx264','-preset','veryfast','-crf','23','-pix_fmt','yuv420p',
                             '-c:a','aac','-b:a','96k','-movflags','+faststart','-y',str(temporary)],timeout=90)
                except subprocess.TimeoutExpired as exc:
                    raise ValueError('Clip non disponibile: tempo massimo di elaborazione superato') from exc
                if not temporary.is_file() or not 0<temporary.stat().st_size<=30_000_000:
                    raise ValueError('Clip vuota o superiore a 30 MB')
                temporary.replace(target)
                # Only prune this component's generated cache files. Preserve
                # open files and the selected clip if Windows denies deletion.
                try:
                    cached=sorted((p for p in self.root.glob('clip-*.mp4') if not p.name.startswith('clip-tmp-')),
                                  key=lambda p:p.stat().st_mtime_ns,reverse=True)
                except OSError:
                    cached=[]  # Cache housekeeping must not invalidate a generated clip.
                for old in cached[32:]:
                    if old!=target:
                        try:old.unlink()
                        except OSError:pass
            finally:
                temporary.unlink(missing_ok=True)
            return target


def frame_reference(guide,step):
    """Select a source/time pair, without changing chronology or runtime rules.

    Keep this policy in parity with guide-presentation.js: explicit frame time,
    direct visual evidence, linked visual inventory, then a spoken reference.
    """
    def valid_time(value):return type(value) in (int,float) and 0<=value<14400
    def video_url(source):
        try:return youtube_url(source.get('url',''))
        except (ValueError,TypeError,AttributeError):return None
    sources={s['id']:s for s in guide.get('sources',[])}
    fallback=set(filter(None,(video_url(s) for s in [*sources.values(),guide.get('source',{})])))
    direct=[e for e in step.get('evidence',[]) if valid_time(e.get('timestamp_seconds'))]
    inventory=[e for e in (guide.get('source_inventory') or {}).get('items',[])
               if step.get('key') in e.get('step_keys',[]) and not e.get('excluded_reason')
               and valid_time(e.get('timestamp_seconds'))]

    def select(items,seconds=None):
        urls=set()
        for item in items:
            source=sources.get(item.get('source_id'),{})
            url=video_url(source)
            if source.get('url') and url is None:return None
            if url:urls.add(url)
        urls=urls or fallback
        if len(urls)!=1 or not items:return None
        return {'url':next(iter(urls)),'seconds':seconds if seconds is not None else items[0]['timestamp_seconds']}

    params=step['action'].get('params',{})
    if 'source_seconds' in params:
        seconds=params['source_seconds']
        if not valid_time(seconds):return None
        matching=[e for e in direct+inventory if e['timestamp_seconds']==seconds]
        return select(matching or direct+inventory or [{'timestamp_seconds':seconds}],seconds)
    for items in ([e for e in direct if e.get('basis')=='visual'],
                  [e for e in inventory if e.get('basis')=='visual'],direct,inventory):
        if items:return select(items)
    return None


def media_references(guide,step):
    """Enumerate step-linked source/time pairs without claiming visual accuracy.

    Audio-derived timestamps are contextual references; ``visual`` means only
    that the author described a visual observation, not that it was verified.
    """
    def valid_time(value):return type(value) in (int,float) and math.isfinite(value) and 0<=value<14400
    def canonical(source):
        try:return youtube_url(source.get('url',''))
        except (ValueError,TypeError,AttributeError):return None
    sources={s['id']:s for s in guide.get('sources',[])}
    known=[(s.get('id'),s,canonical(s)) for s in [*sources.values(),guide.get('source',{})]]
    video_urls={url for _,_,url in known if url}
    direct=[(e,'step') for e in step.get('evidence',[]) if valid_time(e.get('timestamp_seconds'))]
    inventory=[(e,'inventory') for e in (guide.get('source_inventory') or {}).get('items',[])
               if step.get('key') in e.get('step_keys',[]) and not e.get('excluded_reason')
               and valid_time(e.get('timestamp_seconds'))]
    references=[];seen=set();seen_quotes={}
    def add(item,origin,override_url=None):
        source_id=item.get('source_id');source=sources.get(source_id,{})
        if source_id is not None and source_id not in sources:return
        url=override_url or canonical(source)
        # A named unsupported source never borrows an unrelated video.
        if not url and source.get('url'):return
        if not url and source and source.get('type') not in ('youtube',None):return
        if not url and len(video_urls)==1:url=next(iter(video_urls))
        if not url:return
        seconds=item['timestamp_seconds'];basis=item.get('basis','unverified')
        quote=item.get('quote')
        quotation=(url,seconds,source_id,quote) if isinstance(quote,str) and quote else None
        # The same quotation may be repeated in the inventory and step evidence.
        # Keep the stronger explicit/visual source and avoid misleading duplicates.
        if quotation and basis=='unverified' and seen_quotes.get(quotation) in ('explicit','visual'):return
        identity=(url,seconds,basis,source_id,quote)
        if identity in seen:return
        seen.add(identity)
        if quotation:seen_quotes[quotation]=basis
        ref={'id':'r'+str(len(references)),'basis':basis,'origin':origin,'source_id':source_id,
             'source_title':source.get('title') or guide.get('source',{}).get('title') or 'Video',
             'url':url,'seconds':seconds,'clip_start':max(0,seconds-5),'clip_end':min(14400,seconds+10)}
        if valid_time(item.get('end_seconds')) and item['end_seconds']>=seconds:ref['end_seconds']=item['end_seconds']
        if isinstance(item.get('quote'),str):ref['quote']=item['quote'][:4096]
        references.append(ref)
    params=step['action'].get('params',{});explicit=params.get('source_seconds')
    preferred=frame_reference(guide,step)
    has_unknown_source=any(item.get('source_id') is not None and item['source_id'] not in sources
                           for item,_ in direct+inventory)
    if valid_time(explicit) and preferred and not has_unknown_source:
        matching=next(((item,origin) for item,origin in direct+inventory
                       if item['timestamp_seconds']==explicit and canonical(sources.get(item.get('source_id'),{}))==preferred['url']),None)
        add({'timestamp_seconds':explicit,'basis':'declared',**({'source_id':matching[0].get('source_id')} if matching else {})},'declared',preferred['url'])
    for item,origin in sorted(direct+inventory,key=lambda pair:{'visual':0,'explicit':1}.get(pair[0].get('basis'),2)):
        add(item,origin)
    default=next((r['id'] for r in references if preferred and r['url']==preferred['url'] and r['seconds']==preferred['seconds']),None)
    # Bound response size and expose truncation rather than silently claiming complete coverage.
    truncated=len(references)>256
    references=references[:256]
    if default not in {r['id'] for r in references}:default=None
    return {'references':references,'default_reference':default,'truncated':truncated}
