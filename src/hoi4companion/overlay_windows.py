"""External Windows overlay. No game handles with memory access, no game input."""
import argparse
import base64
import ctypes as ct
from ctypes import wintypes as wt
import hashlib
import io
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time
import traceback
from urllib.parse import quote
from urllib.request import urlopen

from .screen_overlay import targets,rectangles
from .overlay_sprites import ResearchSprites

ROOT=Path(__file__).resolve().parents[2]


class Feed:
    def __init__(self,base,guide_id):
        self.base,self.guide_id=base,guide_id;self.lock=threading.Lock();self.closed=threading.Event();self.version=0
        self.value=[];self.updated=0;self.error='Collegamento al runtime…';self.guide_title=''
    def get(self,path):
        with urlopen(self.base+path,timeout=10) as response:return json.load(response)
    def get_bytes(self,path):
        if not path.startswith('/api/sprite/'):raise ValueError('Percorso asset non valido')
        with urlopen(self.base+path,timeout=10) as response:return response.read()
    def select(self,guide_id):
        with self.lock:self.guide_id=guide_id;self.version+=1;self.value=[];self.updated=0
    def snapshot(self):
        with self.lock:return self.version,list(self.value),self.updated,self.error,self.guide_title
    def run(self):
        catalog=None;guide=None;state=None;guide_key=None;event='';event_plan=None
        while not self.closed.is_set():
            try:
                if catalog is None:catalog=self.get('/api/import/catalog')['entities']
                with urlopen(self.base+'/api/state/events',timeout=12) as stream:
                    for raw in stream:
                        if self.closed.is_set():return
                        if raw.startswith(b'event:'):event=raw[6:].strip().decode();continue
                        if not raw.startswith(b'data:'):continue
                        payload=json.loads(raw[5:])
                        with self.lock:guide_id=self.guide_id;version=self.version
                        if event=='heartbeat':
                            with self.lock:
                                if not payload.get('usable'):self.value=[];self.updated=0;self.error='Lettura live non valida'
                                elif guide_key and guide_key[0]==guide_id and not self.error:self.updated=time.monotonic();continue
                            if not payload.get('usable') or state is None:continue
                            state=self.get('/api/state')
                        elif event=='state':state=payload
                        else:continue
                        suffix='?id='+quote(guide_id,safe='') if guide_id else ''
                        key=(guide_id,state.get('guide_revision'))
                        if key!=guide_key:
                            from .guide_events import compile_event_plan
                            guide=self.get('/api/guide'+suffix);guide_key=key
                            event_plan=compile_event_plan(guide,catalog)
                        runtime=self.get('/api/guide/runtime'+suffix)
                        proposed=targets(guide,runtime,catalog,state,prepare_events=True,event_plan=event_plan)
                        with self.lock:
                            if version!=self.version:continue
                            if self.value!=proposed:self.version+=1
                            self.value=proposed;self.updated=time.monotonic();self.guide_title=guide['title']
                            self.error='' if runtime.get('mode')=='live' else 'Partita non valida per questa guida: evidenziazioni sospese'
            except Exception as exc:
                with self.lock:self.value=[];self.updated=0;self.error=str(exc)
                self.closed.wait(2)


class Ocr:
    def __init__(self):
        self.process=subprocess.Popen(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(ROOT/'scripts'/'overlay-ocr.ps1'),'-Worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW)
        self.answers=queue.Queue();threading.Thread(target=self.read,daemon=True).start()
    def read(self):
        try:
            for line in self.process.stdout:
                try:self.answers.put(json.loads(line))
                except ValueError:pass
        finally:self.answers.put({'error':'Motore OCR terminato'})
    def recognize(self,png,token):
        self.process.stdin.write(json.dumps({'png':base64.b64encode(png).decode('ascii'),'token':token})+'\n');self.process.stdin.flush()
        result=self.answers.get(timeout=8)
        if result.get('error'):raise RuntimeError(result['error'])
        if result.get('token')!=token:raise RuntimeError('Risposta OCR precedente: evidenziazioni sospese')
        return result
    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:self.process.kill()


def legacy_main():
    if os.name!='nt':raise SystemExit('Overlay disponibile solo su Windows')
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--port',type=int,default=8772);parser.add_argument('--guide',default='');parser.add_argument('--start',action='store_true');parser.add_argument('--proof',type=Path,help='Save one actual rendered game/overlay frame for verification');args=parser.parse_args()
    if not 1024<=args.port<=65535:raise SystemExit('Porta locale non valida')
    (ROOT/'.local').mkdir(exist_ok=True)
    user=ct.WinDLL('user32',use_last_error=True)
    user.GetForegroundWindow.restype=wt.HWND;user.GetAncestor.argtypes=[wt.HWND,wt.UINT];user.GetAncestor.restype=wt.HWND
    user.GetWindowLongW.argtypes=[wt.HWND,ct.c_int];user.SetWindowLongW.argtypes=[wt.HWND,ct.c_int,ct.c_long]
    user.GetClientRect.argtypes=[wt.HWND,ct.POINTER(wt.RECT)];user.ClientToScreen.argtypes=[wt.HWND,ct.POINTER(wt.POINT)]
    user.GetWindowThreadProcessId.argtypes=[wt.HWND,ct.POINTER(wt.DWORD)]
    user.SetWindowPos.argtypes=[wt.HWND,wt.HWND,ct.c_int,ct.c_int,ct.c_int,ct.c_int,wt.UINT]
    user.ShowWindow.argtypes=[wt.HWND,ct.c_int]
    user.GetWindowDisplayAffinity.argtypes=[wt.HWND,ct.POINTER(wt.DWORD)];user.SetWindowDisplayAffinity.argtypes=[wt.HWND,wt.DWORD]
    try:user.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
    except AttributeError:pass
    import tkinter as tk
    from tkinter import ttk
    from PIL import ImageGrab,ImageEnhance,ImageOps,Image
    # Only query process executable identity. Never call the memory reader.
    kernel=ct.WinDLL('kernel32',use_last_error=True);kernel.OpenProcess.argtypes=[wt.DWORD,wt.BOOL,wt.DWORD];kernel.OpenProcess.restype=wt.HANDLE
    kernel.QueryFullProcessImageNameW.argtypes=[wt.HANDLE,wt.DWORD,wt.LPWSTR,ct.POINTER(wt.DWORD)];kernel.CloseHandle.argtypes=[wt.HANDLE]
    kernel.CreateMutexW.argtypes=[ct.c_void_p,wt.BOOL,wt.LPCWSTR];kernel.CreateMutexW.restype=wt.HANDLE
    instance=kernel.CreateMutexW(None,False,'Local\\HOI4CompanionOverlay-'+hashlib.sha256(str(ROOT).encode()).hexdigest()[:24])
    if not instance:raise ct.WinError(ct.get_last_error())
    if ct.get_last_error()==183:
        kernel.CloseHandle(instance);return
    user.IsWindowVisible.argtypes=[wt.HWND];user.IsWindowVisible.restype=wt.BOOL
    def viewport():
        window=user.GetForegroundWindow();pid=wt.DWORD();user.GetWindowThreadProcessId(window,ct.byref(pid))
        handle=kernel.OpenProcess(0x1000,False,pid.value)
        if not handle:return None
        try:
            name=ct.create_unicode_buffer(32768);size=wt.DWORD(len(name))
            if not kernel.QueryFullProcessImageNameW(handle,0,name,ct.byref(size)) or Path(name.value).name.casefold()!='hoi4.exe':return None
        finally:kernel.CloseHandle(handle)
        rect=wt.RECT();point=wt.POINT()
        if not user.GetClientRect(window,ct.byref(rect)) or not user.ClientToScreen(window,ct.byref(point)):return None
        return (int(window),pid.value,point.x,point.y,rect.right,rect.bottom)
    feed=Feed('http://127.0.0.1:'+str(args.port),args.guide);threading.Thread(target=feed.run,daemon=True).start()
    sprites=ResearchSprites(feed.get,feed.get_bytes)
    root=tk.Tk();root.title('HOI4 Companion · Evidenziazioni nel gioco');root.geometry('540x260')
    ttk.Label(root,text='Overlay sperimentale · solo diciture visibili e univoche').pack(pady=10)
    select=ttk.Combobox(root,state='readonly',width=65);select.pack(padx=12,pady=4)
    try:guides=feed.get('/api/guides')['guides']
    except Exception:guides=[]
    select['values']=[g['title'] for g in guides]
    if guides:
        select.current(next((i for i,g in enumerate(guides) if g['id']==args.guide),0));feed.select(guides[select.current()]['id'])
    select.bind('<<ComboboxSelected>>',lambda event:feed.select(guides[select.current()]['id']))
    guide_answers=queue.Queue()
    def refresh_guides():
        refresh_button.configure(state='disabled')
        def fetch():
            try:guide_answers.put((feed.get('/api/guides')['guides'],None))
            except Exception as exc:guide_answers.put((None,str(exc)))
        threading.Thread(target=fetch,daemon=True).start()
    refresh_button=ttk.Button(root,text='Ricarica guide installate',command=refresh_guides);refresh_button.pack(pady=4)
    enabled=tk.BooleanVar(value=args.start);ttk.Checkbutton(root,text='Attiva cornici sopra HOI4 (non seleziona nulla)',variable=enabled).pack(pady=8)
    notice=tk.StringVar(value='Apri una schermata del gioco. Chiudi questa finestra per terminare.');ttk.Label(root,textvariable=notice,wraplength=510).pack(padx=12,pady=6)
    layer=tk.Toplevel(root);layer.overrideredirect(True);layer.configure(bg='#010101');layer.wm_attributes('-transparentcolor','#010101');layer.wm_attributes('-topmost',True)
    canvas=tk.Canvas(layer,bg='#010101',highlightthickness=0);canvas.pack(fill='both',expand=True);layer.update_idletasks()
    hwnd=user.GetAncestor(layer.winfo_id(),2);user.SetWindowLongW(hwnd,-20,user.GetWindowLongW(hwnd,-20)|0x08000000|0x20|0x80000)
    # Exclusion prevents our own labels becoming input to the next OCR frame.
    if not user.SetWindowDisplayAffinity(hwnd,0x11):raise RuntimeError('Esclusione overlay dalla cattura non supportata')
    user.ShowWindow(hwnd,0);answers=queue.Queue();jobs=queue.Queue(maxsize=1);ocr=Ocr();stop=threading.Event();last=[];busy=False;epoch=0;last_frame=0;drawn_view=None;drawn_version=None;proof_saved=False
    def worker():
        previous=None;cached=None;visual_key=None;cached_visual=[]
        while not stop.is_set():
            try:
                job=jobs.get(timeout=.2)
            except queue.Empty:continue
            token,view,version,proposed=job
            try:
                captured_at=time.monotonic()
                image=ImageGrab.grab(bbox=(view[2],view[3],view[2]+view[4],view[3]+view[5]),all_screens=True)
                if viewport()!=view:raise RuntimeError('Finestra cambiata durante la cattura')
                # Small game fonts require enlarged tiles; never blur/downscale labels.
                # FPS counters and the animated top bar are not panel content.
                digest=hashlib.sha256(image.crop((0,min(80,image.height),image.width,image.height)).tobytes()).digest()
                if digest!=previous:
                    cached={'width':image.width,'height':image.height,'lines':[]}
                    for top in range(0,image.height,720):
                        for left in range(0,image.width,1280):
                            tile=image.crop((left,top,min(image.width,left+1280),min(image.height,top+720)))
                            tile=ImageEnhance.Contrast(ImageOps.grayscale(tile)).enhance(1.4)
                            data=io.BytesIO();tile.resize((tile.width*2,tile.height*2),Image.Resampling.BICUBIC).save(data,format='PNG')
                            part=ocr.recognize(data.getvalue(),token)
                            for line in part['lines']:
                                for word in line['words']:
                                    word.update(x=word['x']/2+left,y=word['y']/2+top,width=word['width']/2,height=word['height']/2)
                                cached['lines'].append(line)
                    previous=digest
                matches=rectangles(cached,proposed)
                sprite_at=time.monotonic()
                key=(digest,json.dumps(proposed,sort_keys=True,ensure_ascii=False))
                if visual_key!=key:
                    nodes=sprites.match(image,proposed,cached)
                    cached_visual=nodes+sprites.navigation(image,proposed,matches+nodes);visual_key=key
                visual=cached_visual
                visual_keys={m['key'] for m in visual}
                matches=[m for m in matches if m['key'] not in visual_keys]+visual
                if args.proof:
                    (ROOT/'.local'/'overlay-recognition.json').write_text(json.dumps({'seconds':time.monotonic()-captured_at,
                        'sprite_seconds':time.monotonic()-sprite_at,'matches':matches,'text':[line['text'] for line in cached['lines']]}),encoding='utf-8')
                    diagnostic=args.proof.with_stem(args.proof.stem+'-input')
                    if not diagnostic.exists():
                        image.save(diagnostic)
                        diagnostic.with_suffix('.json').write_text(json.dumps(cached,ensure_ascii=False),encoding='utf-8')
                if matches:
                    current=ImageGrab.grab(bbox=(view[2],view[3],view[2]+view[4],view[3]+view[5]),all_screens=True)
                    matches=[m for m in matches if all(image.crop(tuple(map(int,rect))).tobytes()==current.crop(tuple(map(int,rect))).tobytes()
                        for rect in [m['rect'],*m.get('evidence_rects',[])])]
                    if viewport()!=view:matches=[]
                    # These pixels were freshly reobserved, including the GUI
                    # landmarks: recognition duration does not age them further.
                    captured_at=time.monotonic()
                answers.put((token,view,version,matches,None,captured_at))
            except Exception as exc:
                if stop.is_set():return
                (ROOT/'.local'/'overlay-worker.err.log').write_text(traceback.format_exc(),encoding='utf-8')
                answers.put((token,view,version,[],str(exc),time.monotonic()))
    threading.Thread(target=worker,daemon=True).start()
    def save_proof(view):
        nonlocal proof_saved
        if proof_saved or viewport()!=view or not last:return
        canvas.update_idletasks();user.SetWindowDisplayAffinity(hwnd,0)
        try:
            ct.WinDLL('dwmapi').DwmFlush()
            ImageGrab.grab(bbox=(view[2],view[3],view[2]+view[4],view[3]+view[5]),all_screens=True,include_layered_windows=True).save(args.proof)
            proof_saved=True
        finally:user.SetWindowDisplayAffinity(hwnd,0x11)
    def tick_body():
        nonlocal busy,epoch,last,last_frame,drawn_view,drawn_version,proof_saved,guides
        while not guide_answers.empty():
            fresh,failure=guide_answers.get();refresh_button.configure(state='normal')
            if failure:notice.set(failure);continue
            current=guides[select.current()]['id'] if guides and select.current()>=0 else ''
            guides=fresh;select['values']=[g['title'] for g in guides]
            if guides:
                index=next((i for i,g in enumerate(guides) if g['id']==current),0);select.current(index)
                if guides[index]['id']!=current:feed.select(guides[index]['id'])
            else:select.set('');feed.select('')
        view=viewport();version,proposed,updated,error,title=feed.snapshot()
        valid=enabled.get() and view is not None and time.monotonic()-updated<12 and not error and bool(proposed)
        # A native map, menu, movement, resize or stale transport hides prior boxes.
        if not valid or (drawn_view is not None and (drawn_view!=view or drawn_version!=version or time.monotonic()-last_frame>2)):
            canvas.delete('all');user.ShowWindow(hwnd,0);last=[]
            status={'host_pid':os.getpid(),'overlay_window':int(hwnd),'visible':False,
                    'matches':[],'error':error or ('inactive_game' if view is None else 'expired_or_disabled'),
                    'foreground_window':int(user.GetForegroundWindow()),'memory_access':False}
            (ROOT/'.local'/'overlay-status.json').write_text(json.dumps(status,ensure_ascii=False,indent=2),encoding='utf-8')
        while not answers.empty():
            token,old_view,old_version,matches,failure,frame_at=answers.get();busy=False
            if not valid or old_view!=view or old_version!=version:continue
            if time.monotonic()-frame_at>2:
                notice.set('Riconoscimento troppo lento: nessuna cornice obsoleta viene mostrata.')
                (ROOT/'.local'/'overlay-status.json').write_text(json.dumps({'visible':False,'matches':[],
                    'error':'recognition_too_slow','seconds':time.monotonic()-frame_at}),encoding='utf-8')
                continue
            canvas.delete('all');last=matches;last_frame=frame_at;drawn_view=view;drawn_version=version
            if matches:
                layer.geometry(f'{view[4]}x{view[5]}{view[2]:+d}{view[3]:+d}');layer.update_idletasks()
                user.SetWindowPos(hwnd,wt.HWND(-1),view[2],view[3],view[4],view[5],0x0010)
                user.ShowWindow(hwnd,4)
                for match in matches:
                    x,y,right,bottom=match['rect'];color='#5ed1ef' if match['status']=='active' and match['panel']!='event' else '#ffdd57'
                    canvas.create_rectangle(x,y,right,bottom,outline=color,width=3)
                    caption=('GUIDA · '+match['title']+' · '+str(match['count'])+(' ricerca' if match['count']==1 else ' ricerche') if match['panel']=='research_tab'
                             else 'GUIDA'+(' · totale '+str(match['quantity'])+' fabbriche' if match.get('quantity') is not None else ''))
                    canvas.create_text(x,max(12,y-9),text=caption,anchor='w',fill=color,font=('Segoe UI',10,'bold'))
                if args.proof and not proof_saved:
                    # Capture real composited pixels, not a synthesized annotation.
                    root.after(100,lambda current=view:save_proof(current))
            else:user.ShowWindow(hwnd,0)
            notice.set(failure or (f'{len(matches)} diciture evidenziate · {title}' if matches else 'Nessun bersaglio visibile e univoco. Nessuna posizione stimata.'))
            status={'host_pid':os.getpid(),'game_window':view[0],'overlay_window':int(hwnd),'foreground_window':int(user.GetForegroundWindow()),'visible':bool(user.IsWindowVisible(hwnd)),'overlay_style':user.GetWindowLongW(hwnd,-20),'canvas_size':[canvas.winfo_width(),canvas.winfo_height()],'guide':title,'target_count':len(proposed),'matches':matches,'error':failure,'basis':'visual_evidence_not_native_widget_identity','frame_at':frame_at,'memory_access':False,'proof_saved':proof_saved}
            (ROOT/'.local'/'overlay-status.json').write_text(json.dumps(status,ensure_ascii=False,indent=2),encoding='utf-8')
        if valid and not busy:
            epoch+=1;busy=True;jobs.put((epoch,view,version,proposed))
        elif not valid:notice.set(error or ('Overlay disattivato' if not enabled.get() else 'Nessuna indicazione attuale per questa guida.' if not proposed else 'Passa a HOI4 per mostrare le cornici.'))
    def tick():
        try:tick_body()
        except Exception:
            canvas.delete('all');user.ShowWindow(hwnd,0)
            notice.set('Errore overlay: cornici sospese. Chiudi e riapri questa finestra.')
            (ROOT/'.local'/'overlay-host.err.log').write_text(traceback.format_exc(),encoding='utf-8')
        finally:
            if not stop.is_set():root.after(500,tick)
    def close():
        stop.set();feed.closed.set();ocr.close();root.destroy()
    root.protocol('WM_DELETE_WINDOW',close);tick()
    try:root.mainloop()
    finally:stop.set();feed.closed.set();ocr.close();kernel.CloseHandle(instance)


def main():
    from .overlay_native import main as native_main
    native_main()


if __name__=='__main__':main()
