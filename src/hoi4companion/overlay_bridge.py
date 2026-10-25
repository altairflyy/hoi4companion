"""Guide/vision compatibility worker for the native overlay.

The Rust host owns capture, OCR and drawing. This worker preserves the current
runtime projection, observes qualified focus GUI identities read-only, and keeps
the existing OpenCV recognition for other panels. Frames travel as
packed BGRA bytes, never as PNG, Base64, files or a PowerShell subprocess.
"""
import argparse
import hashlib
import json
import math
import sys
import threading
import time

from .overlay_windows import Feed
from .overlay_sprites import ResearchSprites
from .screen_overlay import rectangles

SCHEMA_VERSION=1
MAX_HEADER_BYTES=1_048_576
MAX_FRAME_BYTES=80_000_000
MAX_RESPONSE_BYTES=16_000_000


def _integer(value, minimum=0, maximum=2**63-1):
    return type(value) is int and minimum<=value<=maximum


def _strict_json(data):
    def invalid(value):raise ValueError('Valore JSON non finito: '+value)
    return json.loads(data,parse_constant=invalid)


def _ocr(value,width,height):
    if (not isinstance(value,dict) or value.get('width')!=width or value.get('height')!=height
            or not isinstance(value.get('lines'),list) or len(value['lines'])>20000):
        raise ValueError('Documento OCR non valido')
    count=0
    for line in value['lines']:
        if not isinstance(line,dict) or not isinstance(line.get('text'),str) or not isinstance(line.get('words'),list):
            raise ValueError('Riga OCR non valida')
        count+=len(line['words'])
        if count>100000:raise ValueError('Troppe parole OCR')
        for word in line['words']:
            if (not isinstance(word,dict) or not isinstance(word.get('text'),str)
                    or any(type(word.get(k)) not in (int,float) or not math.isfinite(word[k])
                           for k in ('x','y','width','height'))):
                raise ValueError('Parola OCR non valida')
    return value


class Bridge:
    def __init__(self,feed,*,sprites=None,geometry=None):
        self.feed=feed
        self.sprites=sprites if sprites is not None else ResearchSprites(feed.get,feed.get_bytes)
        self.guides=None
        self.cached_key=None
        self.cached_matches=[]
        self.geometry=geometry

    def close(self):
        if self.geometry is not None:self.geometry.close()
        self.feed.closed.set()

    def _guides(self,refresh=False):
        if self.guides is None or refresh:
            rows=self.feed.get('/api/guides').get('guides',[])
            if not isinstance(rows,list):raise ValueError('Elenco guide non valido')
            self.guides=[{'id':row['id'],'title':row['title']} for row in rows
                         if isinstance(row,dict) and isinstance(row.get('id'),str)
                         and isinstance(row.get('title'),str)]
        return list(self.guides)

    def snapshot(self):
        version,proposed,updated,error,title=self.feed.snapshot()
        age=(time.monotonic()-updated)*1000 if updated else None
        usable=bool(updated and age is not None and 0<=age<12000 and not error and proposed)
        return {'version':version,'proposed':proposed if usable else [],'usable':usable,
                'title':title,'error':error or ('' if usable else 'Nessuna indicazione live attuale'),
                'age_ms':age,'guide_id':self.feed.guide_id}

    def handle(self,request,data=b''):
        if not isinstance(request,dict) or request.get('schema_version')!=SCHEMA_VERSION:
            raise ValueError('Versione protocollo overlay non supportata')
        kind=request.get('kind');token=request.get('token',0)
        if not _integer(token):raise ValueError('Token overlay non valido')
        base={'schema_version':SCHEMA_VERSION,'kind':kind,'token':token,'ok':True}
        if kind=='snapshot':
            try:guides=self._guides(bool(request.get('refresh_guides',False)))
            except (OSError,ValueError) as exc:
                return {**base,**self.snapshot(),'usable':False,'proposed':[],
                        'guides':list(self.guides or []),'error':'Server non disponibile: '+str(exc)}
            return {**base,**self.snapshot(),'guides':guides}
        if kind=='select':
            guide=request.get('guide_id')
            if not isinstance(guide,str) or guide not in {row['id'] for row in self._guides()}:
                raise ValueError('Guida non installata')
            if hasattr(self.feed,'activate'):self.feed.activate(guide)
            else:self.feed.select(guide)
            self.cached_key=None;self.cached_matches=[]
            return {**base,**self.snapshot(),'guides':self._guides()}
        if kind=='close':
            self.close()
            return {**base,'usable':False,'matches':[]}
        if kind=='geometry':
            view=request.get('view')
            if (not isinstance(view,list) or len(view)!=6 or any(type(n) is not int for n in view)
                    or not _integer(view[0],1) or not _integer(view[1],1)
                    or not _integer(view[4],1,8192) or not _integer(view[5],1,8192)
                    or view[4]*view[5]*4>MAX_FRAME_BYTES):
                raise ValueError('Identità finestra overlay non valida')
            snapshot=self.snapshot()
            if not snapshot['usable'] or request.get('version')!=snapshot['version']:
                return {**base,**snapshot,'view':view,'usable':False,'native_available':False,'matches':[]}
            if self.geometry is None:
                return {**base,**snapshot,'view':view,'native_available':False,'matches':[]}
            began=time.perf_counter()
            result=self.geometry.read(view,snapshot['proposed'])
            current=self.snapshot()
            usable=current['usable'] and current['version']==snapshot['version']
            available=result.get('available') is True
            allowed={p['key']:p for p in snapshot['proposed']}
            matches=[]
            for match in result.get('matches',[]) if available and usable else []:
                if not isinstance(match,dict):continue
                proposal=allowed.get(match.get('key'))
                if proposal and (match.get('panel'),match.get('identity'))==(proposal['panel'],proposal['identity']):
                    matches.append({**proposal,**match})
                elif match.get('panel')=='research_tab':
                    folder=match.get('identity');keys=match.get('step_keys')
                    expected=[p['key'] for p in snapshot['proposed']
                              if p['panel']=='research' and p.get('folder')==folder]
                    if (isinstance(folder,str) and folder and match.get('folder')==folder
                            and match.get('key')=='research-tab:'+folder
                            and isinstance(keys,list) and keys and keys==expected
                            and len(keys)==len(set(keys)) and type(match.get('count')) is int
                            and match['count']==len(keys)):
                        matches.append(match)
            return {**base,**current,'view':view,'usable':usable,'native_available':available,
                    'matches':matches,'handled_panels':result.get('handled_panels',[]) if available else [],
                    'geometry_ms':(time.perf_counter()-began)*1000,'native_error':result.get('error',''),
                    'diagnostics':result.get('diagnostics',{}),'memory_access':True,
                    'active_panel':result.get('active_panel'),
                    'suspend_visual':result.get('suspend_visual',False)}
        if kind!='frame':raise ValueError('Richiesta overlay non supportata')
        started=time.perf_counter()
        width,height=request.get('width'),request.get('height')
        if not _integer(width,1,8192) or not _integer(height,1,8192):
            raise ValueError('Dimensioni frame overlay non valide')
        expected=width*height*4
        if expected>MAX_FRAME_BYTES or request.get('raw_len')!=expected or len(data)!=expected:
            raise ValueError('Dimensione payload BGRA non valida')
        view=request.get('view')
        if (not isinstance(view,list) or len(view)!=6 or any(type(n) is not int for n in view)
                or view[0]<=0 or view[1]<=0 or view[4:]!=[width,height]):
            raise ValueError('Identità finestra overlay non valida')
        ocr=_ocr(request.get('ocr'),width,height)
        snapshot=self.snapshot()
        if not snapshot['usable'] or request.get('version')!=snapshot['version']:
            return {**base,**snapshot,'view':view,'usable':False,'matches':[],
                    'cache_hit':False,'recognition_ms':0}
        proposed=snapshot['proposed']
        # The host provides a scene hash for diagnostics. Admission uses our
        # own pixel digest so stale/corrupt metadata cannot reuse old matches.
        digest=hashlib.sha256(memoryview(data)[min(80,height)*width*4:]).digest()
        ocr_key=hashlib.sha256(json.dumps(ocr,sort_keys=True,ensure_ascii=False,
                               separators=(',',':'),allow_nan=False).encode()).digest()
        key=(tuple(view),snapshot['version'],digest,ocr_key)
        hit=key==self.cached_key
        if hit:
            matches=self.cached_matches
        else:
            from PIL import Image
            image=Image.frombytes('RGB',(width,height),data,'raw','BGRX')
            matches=rectangles(ocr,proposed)
            visual=self.sprites.match(image,proposed,ocr)
            visual+=self.sprites.navigation(image,proposed,matches+visual)
            visual_keys={row['key'] for row in visual}
            matches=[row for row in matches if row['key'] not in visual_keys]+visual
            self.cached_key=key;self.cached_matches=matches
        current=self.snapshot()
        usable=current['usable'] and current['version']==snapshot['version']
        return {**base,**current,'view':view,'usable':usable,
                'matches':matches if usable else [],'cache_hit':hit,
                'recognition_ms':(time.perf_counter()-started)*1000}


def _read_exact(stream,length):
    chunks=[];remaining=length
    while remaining:
        chunk=stream.read(remaining)
        if not chunk:raise EOFError('Frame BGRA troncato')
        chunks.append(chunk);remaining-=len(chunk)
    return b''.join(chunks)


def _write(stream,value):
    data=json.dumps(value,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode('utf8')
    if len(data)>MAX_RESPONSE_BYTES:
        data=b'{"schema_version":1,"ok":false,"usable":false,"matches":[],"error":"Risposta overlay oltre il limite"}'
    stream.write(data+b'\n');stream.flush()


def serve(bridge,source,target):
    try:
        while True:
            request={}
            try:
                header=source.readline(MAX_HEADER_BYTES+1)
                if not header:return
                if len(header)>MAX_HEADER_BYTES or not header.endswith(b'\n'):
                    raise ValueError('Intestazione overlay oltre il limite o troncata')
                request=_strict_json(header)
                if not isinstance(request,dict):raise ValueError('Richiesta overlay non valida')
                raw_len=request.get('raw_len',0)
                if not _integer(raw_len,0,MAX_FRAME_BYTES) or raw_len and request.get('kind')!='frame':
                    raise ValueError('Dimensione payload overlay non valida')
                data=_read_exact(source,raw_len)
            except (ValueError,UnicodeError,EOFError) as exc:
                _write(target,{'schema_version':1,'kind':request.get('kind') if isinstance(request,dict) else None,
                               'token':request.get('token',0) if isinstance(request,dict) else 0,
                               'ok':False,'usable':False,'matches':[],'error':str(exc)})
                return
            try:result=bridge.handle(request,data)
            except Exception as exc:
                result={'schema_version':1,'kind':request.get('kind'),'token':request.get('token',0),
                        'ok':False,'usable':False,'matches':[],'error':str(exc)}
            _write(target,result)
            if request.get('kind')=='close':return
    finally:bridge.close()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port',type=int,default=8766)
    parser.add_argument('--guide',default='')
    args=parser.parse_args()
    if not 1024<=args.port<=65535:parser.error('Porta locale non valida')
    feed=Feed('http://127.0.0.1:'+str(args.port),args.guide)
    from .overlay_geometry import NativeGeometry
    bridge=Bridge(feed,geometry=NativeGeometry(feed.get))
    # Browser and HUD share the server selection, including across restarts.
    try:
        guides=bridge._guides()
        if args.guide:feed.activate(args.guide)
        else:feed.select(feed.get('/api/guide/active')['guide_id'])
    except Exception:
        # Server outages remain explicit in Feed; the binary protocol survives.
        bridge.guides=[]
    threading.Thread(target=feed.run,daemon=True).start()
    serve(bridge,sys.stdin.buffer,sys.stdout.buffer)


if __name__=='__main__':main()
