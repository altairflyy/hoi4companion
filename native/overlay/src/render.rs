//! Native external Direct2D overlay. No injected hooks, game input, or process-memory APIs.
//!
//! Rendering and native controls live on one dedicated STA/message-loop thread.
//! Callers only publish the latest frame, so a busy renderer cannot queue stale
//! highlights. The safety timer hides independently of recognition progress.
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::{Interface, PCWSTR, w};
use windows::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection,
    CreateFontIndirectW, DeleteDC, DeleteObject, GetDC, GetSysColorBrush, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, COLOR_WINDOW, DIB_RGB_COLORS, HFONT, LOGFONTW, SRCCOPY};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK,SetWinEventHook,UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::{Shell_NotifyIconW,NOTIFYICONDATAW,NIF_MESSAGE,NIF_ICON,NIF_TIP,NIM_ADD,NIM_DELETE};
use crate::wake::Wake;

thread_local! {static FOREGROUND_WAKE:std::cell::RefCell<Option<Arc<Wake>>>=const {std::cell::RefCell::new(None)};}
unsafe extern "system" fn foreground_changed(_hook:HWINEVENTHOOK,_event:u32,_window:HWND,
    _object:i32,_child:i32,_thread:u32,_time:u32) {
    FOREGROUND_WAKE.with(|slot|{if let Some(wake)=slot.borrow().as_ref(){wake.notify();}});
}
struct ForegroundEvents(HWINEVENTHOOK);
impl Drop for ForegroundEvents {
    fn drop(&mut self){unsafe {if !self.0.0.is_null(){let _=UnhookWinEvent(self.0);}}
        FOREGROUND_WAKE.with(|slot|*slot.borrow_mut()=None);}
}

const UPDATE: u32 = WM_APP + 33;
const STOP: u32 = WM_APP + 34;
const TRAY: u32 = WM_APP + 35;
const PAUSE: usize = 201;
const QUIT: usize = 202;
const GUIDES: usize = 203;
const REFRESH: usize = 204;
const MAX_MARKS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct TargetWindow {
    pub hwnd: isize,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    pub rect: [f32; 4],
    pub caption: String,
    pub active: bool,
    pub choice: bool,
    pub panel: String,
}

fn overlaps(a:[f32;4],b:[f32;4],gap:f32)->bool {
    a[0]<b[2]+gap && a[2]+gap>b[0] && a[1]<b[3]+gap && a[3]+gap>b[1]
}

/// Place captions outside all highlighted controls. Production uses a compact
/// side column, rather than covering the previous line's name/factory counters.
/// When no safe space remains, keep the border and omit its caption.
fn caption_layout(frame:&OverlayFrame,scale:f32)->Vec<Option<[f32;4]>> {
    let width=frame.target.width as f32;let height=frame.target.height as f32;
    let gap=6.0*scale;
    let production_right=frame.marks.iter().filter(|m|m.panel=="production")
        .map(|m|m.rect[2]).fold(0.0_f32,f32::max);
    let mut placed=Vec::new();let mut result=Vec::new();
    for mark in &frame.marks {
        let production=mark.panel=="production";
        if mark.caption.is_empty() || production&&mark.active {result.push(None);continue;}
        let label_height=if production {20.0}else{25.0}*scale;
        let label_width=((mark.caption.chars().count() as f32*if production {6.5}else{7.6}+16.0)*scale)
            .min(if production {300.0}else{460.0}*scale).min(width);
        let [left,top,right,bottom]=mark.rect;
        let mut candidates=Vec::new();
        if production {
            // The outer edge of the full production list keeps every caption
            // away from the icons, model names and factory assignment widgets.
            let x=production_right+gap;
            if x+label_width<=width {
                let y=top.max(90.0*scale).min((height-label_height).max(0.0));
                candidates.push([x,y,x+label_width,y+label_height]);
                let mut y=90.0*scale;
                while y+label_height<=height {
                    candidates.push([x,y,x+label_width,y+label_height]);
                    y+=label_height+gap;
                }
            }
        }else{
            let x=left.min(width-label_width).max(0.0);
            candidates.extend([
                [x,top-label_height-gap,x+label_width,top-gap],
                [right+gap,top,right+gap+label_width,top+label_height],
                [x,bottom+gap,x+label_width,bottom+gap+label_height],
                [left-gap-label_width,top,left-gap,top+label_height],
            ]);
        }
        let selected=candidates.into_iter().find(|r|
            r[0]>=0.0&&r[1]>=0.0&&r[2]<=width&&r[3]<=height&&
            !frame.marks.iter().any(|other|overlaps(*r,other.rect,gap/2.0))&&
            !placed.iter().any(|other|overlaps(*r,*other,gap/2.0)));
        if let Some(rect)=selected {placed.push(rect);}
        result.push(selected);
    }
    result
}

#[derive(Clone, Debug)]
pub struct OverlayFrame {
    pub target: TargetWindow,
    pub marks: Vec<Highlight>,
    pub observed_at: Instant,
    pub valid_for: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlAction {
    SelectGuide(String),
    RefreshGuides,
    OpenSuite,
}

#[derive(Default)]
struct Shared {
    desired: Option<OverlayFrame>,
    status: String,
    guides: Vec<(String, String)>,
    selected: String,
    actions: VecDeque<ControlAction>,
    excluded: bool,
    revision: u64,
    foreground_events:bool,
    tray_controls:bool,
}

impl Shared {
    fn set_guides(&mut self,guides:Vec<(String,String)>,selected:&str)->bool {
        let guides:Vec<_>=guides.into_iter().take(256).collect();
        if self.guides==guides && self.selected==selected {return false;}
        self.guides=guides;self.selected=selected.to_owned();true
    }
    fn set_status(&mut self,text:String)->bool {
        if self.status==text {return false;}
        self.status=text;true
    }
    fn publish(&mut self,frame:OverlayFrame) {
        // A new witnessed image renews the safety deadline. It is not a visual
        // change: keep the existing composition surface until geometry, content
        // or target identity changes. UI status/guide refreshes use the same rule.
        let changed=self.desired.as_ref().is_none_or(|previous|
            previous.target!=frame.target || previous.marks!=frame.marks);
        self.desired=Some(frame);
        if changed {self.revision=self.revision.wrapping_add(1);}
    }
    fn invalidate(&mut self)->bool {
        let changed=self.desired.take().is_some();
        if changed {self.revision=self.revision.wrapping_add(1);}
        changed
    }
}

pub struct Renderer {
    hwnd: isize,
    layer: isize,
    shared: Arc<Mutex<Shared>>,
    paused: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    wake:Arc<Wake>,
}

fn hwnd(value: isize) -> HWND { HWND(value as *mut core::ffi::c_void) }
fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

impl Renderer {
    pub fn new(discreet:bool) -> Result<Self, String> {
        let shared=Arc::new(Mutex::new(Shared { excluded:true, ..Shared::default() }));
        let paused=Arc::new(AtomicBool::new(false));
        let exit=Arc::new(AtomicBool::new(false));
        let wake=Arc::new(Wake::default());
        let (ready, receiver)=mpsc::sync_channel(1);
        let (s,p,e)=(shared.clone(),paused.clone(),exit.clone());
        let events=wake.clone();
        let thread=thread::Builder::new().name("overlay-window".into()).spawn(move || {
            let result=unsafe { run(s,p,e.clone(),events,discreet,&ready) };
            if let Err(error)=result { let _=ready.send(Err(error)); }
            e.store(true,Ordering::Release);
        }).map_err(|e|e.to_string())?;
        match receiver.recv_timeout(Duration::from_secs(15)).map_err(|e|e.to_string())? {
            Ok((window,layer))=>Ok(Self { hwnd:window,layer,shared,paused,exit,thread:Some(thread),wake }),
            Err(error)=>{let _=thread.join();Err(error)}
        }
    }
    fn update(&self) {
        unsafe { let _=PostMessageW(Some(hwnd(self.hwnd)),UPDATE,WPARAM(0),LPARAM(0)); }
    }
    pub fn show(&self, frame:OverlayFrame) -> Result<(),String> {
        validate_frame(&frame)?;
        let mut shared=self.shared.lock().map_err(|_|"Overlay state poisoned")?;
        shared.publish(frame);
        drop(shared);self.update();Ok(())
    }
    pub fn hide(&self) {
        let changed=self.shared.lock().map(|mut state|state.invalidate()).unwrap_or(true);
        if changed {self.update();}
    }
    pub fn set_status(&self,text:&str) {
        let localized=crate::i18n::text(text);let text=localized.as_str();
        let changed=self.shared.lock().map(|mut s|s.set_status(text.chars().take(300).collect())).unwrap_or(false);
        if changed {self.update();}
    }
    pub fn set_guides(&self,guides:Vec<(String,String)>,selected_id:&str) {
        let changed=self.shared.lock().map(|mut s|s.set_guides(guides,selected_id)).unwrap_or(false);
        if changed {self.update();}
    }
    pub fn take_action(&self)->Option<ControlAction> {
        self.shared.lock().ok()?.actions.pop_front()
    }
    pub fn set_capture_excluded(&self,excluded:bool) {
        if let Ok(mut s)=self.shared.lock() {s.excluded=excluded;}
        self.update();
    }
    pub fn is_paused(&self)->bool {self.paused.load(Ordering::Acquire)}
    pub fn wake_revision(&self)->u64 {self.wake.revision()}
    pub fn foreground_events(&self)->bool {self.shared.lock().map(|s|s.foreground_events).unwrap_or(false)}
    pub fn wait_idle(&self,previous:u64,timeout:Duration) {self.wake.wait(previous,timeout);}
    pub fn set_paused(&self,paused:bool) {
        self.paused.store(paused,Ordering::Release);self.update();
    }
    pub fn should_exit(&self)->bool {self.exit.load(Ordering::Acquire)}
    pub fn overlay_window(&self)->isize {self.layer}
    pub fn control_window(&self)->isize {self.hwnd}
    pub fn controls_in_tray(&self)->bool {self.shared.lock().is_ok_and(|s|s.tray_controls)}
    pub fn shutdown(&mut self) {
        if self.thread.is_none() {return;}
        if !self.exit.load(Ordering::Acquire) {
            unsafe {let _=PostMessageW(Some(hwnd(self.hwnd)),STOP,WPARAM(0),LPARAM(0));}
        }
        if let Some(thread)=self.thread.take() {let _=thread.join();}
    }
}

impl Drop for Renderer {fn drop(&mut self) {self.shutdown();}}

fn validate_frame(frame:&OverlayFrame)->Result<(),String> {
    if frame.target.hwnd==0 || frame.target.pid==0 || frame.target.width==0 || frame.target.height==0
        || frame.target.width>16384 || frame.target.height>16384 || frame.marks.len()>MAX_MARKS
        || frame.valid_for.is_zero() || frame.valid_for>Duration::from_secs(2) {
        return Err("Invalid overlay frame bounds or lifetime".into());
    }
    for mark in &frame.marks {
        let [left,top,right,bottom]=mark.rect;
        if !mark.rect.iter().all(|n|n.is_finite()) || left<0.0 || top<0.0 || right<=left || bottom<=top
            || right>frame.target.width as f32 || bottom>frame.target.height as f32 {
            return Err("Invalid overlay rectangle".into());
        }
    }
    Ok(())
}

unsafe fn target_matches(target:&TargetWindow)->bool {
    unsafe {
        let window=hwnd(target.hwnd);
        if GetForegroundWindow()!=window || IsIconic(window).as_bool() || !IsWindowVisible(window).as_bool() {return false;}
        let mut pid=0;
        GetWindowThreadProcessId(window,Some(&mut pid));
        if pid!=target.pid {return false;}
        let mut rect=RECT::default();let mut point=POINT::default();
        GetClientRect(window,&mut rect).is_ok() && ClientToScreen(window,&mut point).as_bool()
            && point.x==target.x && point.y==target.y && rect.right-rect.left==target.width as i32
            && rect.bottom-rect.top==target.height as i32
    }
}

/// Capture real compositor output only for explicit diagnostics. The caller
/// temporarily disables this process's overlay capture exclusion first, then
/// restores it. This never synthesizes marks into the captured image.
pub fn save_proof(target:&TargetWindow,path:&Path)->Result<(),String> {
    if target.width==0 || target.height==0 || target.width as u64*target.height as u64>40_000_000 {
        return Err("Invalid proof capture bounds".into());
    }
    unsafe {
        if !target_matches(target) {return Err("Target changed before proof capture".into());}
        DwmFlush().map_err(|e|e.to_string())?;
        let display=GetDC(None);
        if display.0.is_null() {return Err(windows::core::Error::from_win32().to_string());}
        let memory=CreateCompatibleDC(Some(display));
        if memory.0.is_null() {ReleaseDC(None,display);return Err(windows::core::Error::from_win32().to_string());}
        let info=BITMAPINFO {bmiHeader:BITMAPINFOHEADER {biSize:std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth:target.width as i32,biHeight:-(target.height as i32),biPlanes:1,biBitCount:32,
            biCompression:BI_RGB.0,..Default::default()},..Default::default()};
        let mut pixels=std::ptr::null_mut();
        let bitmap=match CreateDIBSection(Some(display),&info,DIB_RGB_COLORS,&mut pixels,None,0) {
            Ok(bitmap)=>bitmap,
            Err(error)=>{let _=DeleteDC(memory);ReleaseDC(None,display);return Err(error.to_string());}
        };
        let old=SelectObject(memory,bitmap.into());
        let result=(|| {
            BitBlt(memory,0,0,target.width as i32,target.height as i32,Some(display),target.x,target.y,SRCCOPY|CAPTUREBLT)
                .map_err(|e|e.to_string())?;
            if !target_matches(target) {return Err("Target changed during proof capture".into());}
            if pixels.is_null() {return Err("Proof capture returned no pixels".into());}
            let mut bytes=std::slice::from_raw_parts(pixels as *const u8,target.width as usize*target.height as usize*4).to_vec();
            for pixel in bytes.as_chunks_mut::<4>().0 {pixel.swap(0,2);pixel[3]=255;}
            image::save_buffer_with_format(path,&bytes,target.width,target.height,image::ColorType::Rgba8,image::ImageFormat::Png)
                .map_err(|e|e.to_string())
        })();
        SelectObject(memory,old);let _=DeleteObject(bitmap.into());let _=DeleteDC(memory);ReleaseDC(None,display);
        result
    }
}

/// Direct2D strokes are centered on their path. Inset that path so the painted
/// border stays inside the actual clipped widget, including fractional zoom.
fn border_width(rect:[f32;4],requested:f32)->f32 {
    requested.min((rect[2]-rect[0]).min(rect[3]-rect[1])/2.0)
}
fn border_path(rect:[f32;4],stroke:f32,scale:f32)->D2D1_ROUNDED_RECT {
    let [left,top,right,bottom]=rect;
    let inset=stroke/2.0;
    let rect=D2D_RECT_F {left:left+inset,top:top+inset,right:right-inset,bottom:bottom-inset};
    let width=(rect.right-rect.left).max(0.0);let height=(rect.bottom-rect.top).max(0.0);
    let radius=(5.0*scale).min(width/2.0).min(height/2.0);
    D2D1_ROUNDED_RECT {rect,radiusX:radius,radiusY:radius}
}

struct Paint {
    strokes:[ID2D1SolidColorBrush;3],
    fills:[ID2D1SolidColorBrush;3],
    shadow:ID2D1SolidColorBrush,
    label_bg:ID2D1SolidColorBrush,
}
impl Paint {
    unsafe fn new(context:&ID2D1DeviceContext)->windows::core::Result<Self> {
        unsafe {
            let colors=[D2D1_COLOR_F {r:1.0,g:0.83,b:0.30,a:1.0},
                        D2D1_COLOR_F {r:0.27,g:0.84,b:1.0,a:1.0},
                        D2D1_COLOR_F {r:0.78,g:0.59,b:1.0,a:1.0}];
            let stroke=|i:usize|context.CreateSolidColorBrush(&colors[i],None);
            let fill=|i:usize|context.CreateSolidColorBrush(&D2D1_COLOR_F {a:0.09,..colors[i]},None);
            Ok(Self {strokes:[stroke(0)?,stroke(1)?,stroke(2)?],fills:[fill(0)?,fill(1)?,fill(2)?],
                shadow:context.CreateSolidColorBrush(&D2D1_COLOR_F {r:0.01,g:0.02,b:0.03,a:0.85},None)?,
                label_bg:context.CreateSolidColorBrush(&D2D1_COLOR_F {r:0.035,g:0.055,b:0.075,a:0.82},None)?})
        }
    }
}

struct Graphics {
    context: ID2D1DeviceContext,
    swap: IDXGISwapChain1,
    _composition: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    write: IDWriteFactory,
    paint: Paint,
    formats: Option<(f32,IDWriteTextFormat,IDWriteTextFormat)>,
    bitmap: Option<ID2D1Bitmap1>,
    size:(u32,u32),
}

impl Graphics {
    unsafe fn new(window:HWND)->windows::core::Result<Self> {
        unsafe {
            let mut device=None;
            D3D11CreateDevice(None,D3D_DRIVER_TYPE_HARDWARE,HMODULE::default(),D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,D3D11_SDK_VERSION,Some(&mut device),None,None)?;
            let device:ID3D11Device=device.unwrap();
            let dxgi:IDXGIDevice=device.cast()?;
            let adapter=dxgi.GetAdapter()?;
            let factory:IDXGIFactory2=adapter.GetParent()?;
            let descriptor=DXGI_SWAP_CHAIN_DESC1 {Width:1,Height:1,Format:DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc:DXGI_SAMPLE_DESC {Count:1,Quality:0},BufferUsage:DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount:2,Scaling:DXGI_SCALING_STRETCH,SwapEffect:DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode:DXGI_ALPHA_MODE_PREMULTIPLIED,..Default::default()};
            let swap=factory.CreateSwapChainForComposition(&device,&descriptor,None)?;
            let factory:ID2D1Factory1=D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,None)?;
            let d2d=factory.CreateDevice(&dxgi)?;
            let context=d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            context.SetUnitMode(D2D1_UNIT_MODE_PIXELS);
            context.SetDpi(96.0,96.0);
            let composition:IDCompositionDevice=DCompositionCreateDevice(&dxgi)?;
            let target=composition.CreateTargetForHwnd(window,true)?;
            let visual=composition.CreateVisual()?;
            visual.SetContent(&swap)?;target.SetRoot(&visual)?;composition.Commit()?;
            let write:IDWriteFactory=DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let paint=Paint::new(&context)?;
            Ok(Self {context,swap,_composition:composition,_target:target,_visual:visual,write,paint,formats:None,bitmap:None,size:(0,0)})
        }
    }
    unsafe fn size(&mut self,width:u32,height:u32)->windows::core::Result<()> {
        unsafe {
            if self.size==(width,height) {return Ok(());}
            self.context.SetTarget(None::<&ID2D1Image>);self.bitmap=None;
            self.swap.ResizeBuffers(2,width,height,DXGI_FORMAT_B8G8R8A8_UNORM,DXGI_SWAP_CHAIN_FLAG(0))?;
            let surface:IDXGISurface=self.swap.GetBuffer(0)?;
            let properties=D2D1_BITMAP_PROPERTIES1 {pixelFormat:D2D1_PIXEL_FORMAT {
                format:DXGI_FORMAT_B8G8R8A8_UNORM,alphaMode:D2D1_ALPHA_MODE_PREMULTIPLIED},
                dpiX:96.0,dpiY:96.0,bitmapOptions:D2D1_BITMAP_OPTIONS_TARGET|D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()};
            let bitmap=self.context.CreateBitmapFromDxgiSurface(&surface,Some(&properties))?;
            self.context.SetTarget(&bitmap);self.bitmap=Some(bitmap);self.size=(width,height);Ok(())
        }
    }
    unsafe fn draw(&mut self,frame:&OverlayFrame)->windows::core::Result<()> {
        unsafe {
            self.size(frame.target.width,frame.target.height)?;
            let scale=(GetDpiForWindow(hwnd(frame.target.hwnd)) as f32/96.0).clamp(1.0,3.0);
            if self.formats.as_ref().is_none_or(|(previous,_,_)|*previous!=scale) {
            let format=self.write.CreateTextFormat(w!("Segoe UI"),None,DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,14.0*scale,w!("it-IT"))?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            let compact=self.write.CreateTextFormat(w!("Segoe UI"),None,DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,12.0*scale,w!("it-IT"))?;
            compact.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            compact.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            self.formats=Some((scale,format,compact));
            }
            let (_,format,compact)=self.formats.as_ref().unwrap();
            let captions=caption_layout(frame,scale);
            self.context.BeginDraw();self.context.Clear(Some(&D2D1_COLOR_F {r:0.0,g:0.0,b:0.0,a:0.0}));
            let result={
                for (index,mark) in frame.marks.iter().enumerate() {
                    let tone=if mark.choice {2}else if mark.active && mark.panel!="event" {1}else{0};
                    let stroke=&self.paint.strokes[tone];let fill=&self.paint.fills[tone];
                    let width=border_width(mark.rect,if mark.panel=="production" {1.4}else{1.8}*scale);
                    let round=border_path(mark.rect,width,scale);
                    if mark.panel!="production" {
                        let shadow_width=border_width(mark.rect,4.0*scale);
                        self.context.DrawRoundedRectangle(&border_path(mark.rect,shadow_width,scale),&self.paint.shadow,shadow_width,None);
                        self.context.FillRoundedRectangle(&round,fill);
                    }
                    self.context.DrawRoundedRectangle(&round,stroke,width,None);
                    let caption:Vec<u16>=mark.caption.chars().filter(|c|!c.is_control()).take(150).collect::<String>().encode_utf16().collect();
                    if caption.is_empty() {continue;}
                    let Some([left,top,right,bottom])=captions[index] else {continue;};
                    let label=D2D_RECT_F {left,top,right,bottom};
                    self.context.FillRoundedRectangle(&D2D1_ROUNDED_RECT {rect:label,radiusX:4.0*scale,radiusY:4.0*scale},&self.paint.label_bg);
                    let label_text=D2D_RECT_F {left:label.left+8.0*scale,..label};
                    self.context.DrawText(&caption,if mark.panel=="production" {compact}else{format},&label_text,stroke,D2D1_DRAW_TEXT_OPTIONS_CLIP,DWRITE_MEASURING_MODE_NATURAL);
                }
                Ok::<(),windows::core::Error>(())
            };
            let finished=self.context.EndDraw(None,None);
            result?;finished?;self.swap.Present(0,DXGI_PRESENT(0)).ok()?;Ok(())
        }
    }
}

struct Ui {
    overlay:HWND,
    status:HWND,
    pause:HWND,
    combo:HWND,
    shared:Arc<Mutex<Shared>>,
    paused:Arc<AtomicBool>,
    exit:Arc<AtomicBool>,
    graphics:Option<Graphics>,
    last_revision:u64,
    last_guides:Vec<(String,String)>,
    last_selected:String,
    last_status:String,
    excluded:bool,
    visible:bool,
    _font:HFONT,
    last_paused:bool,
    wake:Arc<Wake>,
}

impl Ui {
    unsafe fn update(&mut self,redraw:bool) {
        unsafe {
            let Ok(shared)=self.shared.lock() else {self.hide();return;};
            let frame=shared.desired.clone();let revision=shared.revision;
            let guides=shared.guides.clone();let selected=shared.selected.clone();let status=shared.status.clone();let excluded=shared.excluded;
            drop(shared);
            let paused=self.paused.load(Ordering::Acquire);
            if paused!=self.last_paused {
                let _=SetWindowTextW(self.pause,PCWSTR(wide(&crate::i18n::text(if paused {"Resume"}else{"Pause"})).as_ptr()));self.last_paused=paused;
            }
            if status!=self.last_status {
                let text=wide(&status);let _=SetWindowTextW(self.status,PCWSTR(text.as_ptr()));self.last_status=status;
            }
            if guides!=self.last_guides || selected!=self.last_selected {
                SendMessageW(self.combo,CB_RESETCONTENT,None,None);
                let mut index=None;
                for (i,(id,title)) in guides.iter().enumerate() {
                    let title=wide(title);SendMessageW(self.combo,CB_ADDSTRING,None,Some(LPARAM(title.as_ptr() as isize)));
                    if *id==selected {index=Some(i);}
                }
                if let Some(i)=index {SendMessageW(self.combo,CB_SETCURSEL,Some(WPARAM(i)),None);}
                self.last_guides=guides;self.last_selected=selected;
            }
            if self.excluded!=excluded {
                if SetWindowDisplayAffinity(self.overlay,if excluded {WDA_EXCLUDEFROMCAPTURE}else{WDA_NONE}).is_err() {
                    self.hide();self.exit.store(true,Ordering::Release);PostQuitMessage(1);return;
                }
                self.excluded=excluded;
            }
            let Some(frame)=frame else {self.hide();return;};
            if self.paused.load(Ordering::Acquire) || frame.observed_at.elapsed()>frame.valid_for
                || frame.marks.is_empty() || !target_matches(&frame.target) {
                self.hide();return;
            }
            if redraw || revision!=self.last_revision || !self.visible {
                let result=(|| {
                    SetWindowPos(self.overlay,Some(HWND_TOPMOST),frame.target.x,frame.target.y,
                        frame.target.width as i32,frame.target.height as i32,SWP_NOACTIVATE)?;
                    self.graphics.as_mut().unwrap().draw(&frame)?;
                    if !target_matches(&frame.target) || frame.observed_at.elapsed()>frame.valid_for {
                        return Ok(false);
                    }
                    let _=ShowWindow(self.overlay,SW_SHOWNOACTIVATE);Ok::<bool,windows::core::Error>(true)
                })();
                match result {
                    Ok(true)=>{},
                    Ok(false)=>{self.hide();return;},
                    Err(error)=>{
                        self.hide();let message=wide(&crate::i18n::format("Rendering suspended: {0}",&[&error.to_string()]));
                        let _=SetWindowTextW(self.status,PCWSTR(message.as_ptr()));return;
                    }
                }
                self.last_revision=revision;self.visible=true;
            }
        }
    }
    unsafe fn hide(&mut self) {unsafe {let _=ShowWindow(self.overlay,SW_HIDE);self.visible=false;}}
    unsafe fn command(&mut self,id:usize,notification:u16) {
        unsafe {
            match id {
                PAUSE=>{
                    let paused=!self.paused.load(Ordering::Acquire);self.paused.store(paused,Ordering::Release);
                    let _=SetWindowTextW(self.pause,PCWSTR(wide(&crate::i18n::text(if paused {"Resume"}else{"Pause"})).as_ptr()));
                    self.update(false);
                }
                QUIT=>{self.exit.store(true,Ordering::Release);PostQuitMessage(0);}
                REFRESH=>{if let Ok(mut s)=self.shared.lock() && s.actions.len()<32 {s.actions.push_back(ControlAction::RefreshGuides);}}
                GUIDES if notification==CBN_SELCHANGE as u16=>{
                    let index=SendMessageW(self.combo,CB_GETCURSEL,None,None).0;
                    if index>=0 && let Some((id,_))=self.last_guides.get(index as usize) {
                            if let Ok(mut s)=self.shared.lock() {
                                if s.actions.len()<32 {s.actions.push_back(ControlAction::SelectGuide(id.clone()));}
                                s.desired=None;s.revision=s.revision.wrapping_add(1);
                            }
                            self.hide();
                    }
                }
                _=>{}
            }
            self.wake.notify();
        }
    }
}

unsafe extern "system" fn window_proc(window:HWND,message:u32,wparam:WPARAM,lparam:LPARAM)->LRESULT {
    unsafe {
        let pointer=GetWindowLongPtrW(window,GWLP_USERDATA) as *mut Ui;
        if !pointer.is_null() {
            let ui=&mut *pointer;
            match message {
                // UPDATE includes guide/status and evidence timestamp refreshes.
                // The visual revision decides whether presenting is necessary.
                UPDATE=>{ui.update(false);return LRESULT(0);}
                WM_TIMER=>{ui.update(false);return LRESULT(0);}
                TRAY=>{
                    if lparam.0 as u32==WM_LBUTTONUP {
                        if let Ok(mut s)=ui.shared.lock() && s.actions.len()<32 && !s.actions.contains(&ControlAction::OpenSuite){s.actions.push_back(ControlAction::OpenSuite);}
                        ui.wake.notify();
                    }else if lparam.0 as u32==WM_RBUTTONUP {let _=ShowWindow(window,SW_SHOWNORMAL);}
                    return LRESULT(0);
                }
                STOP|WM_CLOSE=>{ui.exit.store(true,Ordering::Release);PostQuitMessage(0);return LRESULT(0);}
                WM_COMMAND=>{ui.command(wparam.0&0xffff,(wparam.0>>16) as u16);return LRESULT(0);}
                WM_NCHITTEST if window==ui.overlay=>return LRESULT(HTTRANSPARENT as isize),
                WM_MOUSEACTIVATE if window==ui.overlay=>return LRESULT(MA_NOACTIVATE as isize),
                WM_ERASEBKGND if window==ui.overlay=>return LRESULT(1),
                _=>{}
            }
        }
        DefWindowProcW(window,message,wparam,lparam)
    }
}

unsafe fn run(shared:Arc<Mutex<Shared>>,paused:Arc<AtomicBool>,exit:Arc<AtomicBool>,
    wake:Arc<Wake>,discreet:bool,
              ready:&mpsc::SyncSender<Result<(isize,isize),String>>)->Result<(),String> {
    unsafe {
        CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;
        let _com=ComGuard;
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance:HINSTANCE=GetModuleHandleW(None).map_err(|e|e.to_string())?.into();
        let class=WNDCLASSW {lpfnWndProc:Some(window_proc),hInstance:instance,lpszClassName:w!("HOI4CompanionNativeOverlay"),
            hbrBackground:GetSysColorBrush(COLOR_WINDOW),..Default::default()};
        if RegisterClassW(&class)==0 {return Err(windows::core::Error::from_win32().to_string());}
        let overlay=CreateWindowExW(WS_EX_TOPMOST|WS_EX_TRANSPARENT|WS_EX_LAYERED|WS_EX_NOACTIVATE|WS_EX_NOREDIRECTIONBITMAP,
            class.lpszClassName,PCWSTR(wide(&crate::i18n::text("HOI4 Companion · Highlights")).as_ptr()),WS_POPUP,0,0,1,1,None,None,Some(instance),None).map_err(|e|e.to_string())?;
        let control=CreateWindowExW(WINDOW_EX_STYLE(0),class.lpszClassName,PCWSTR(wide(&crate::i18n::text("HOI4 Companion · Native overlay")).as_ptr()),
            WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX,CW_USEDEFAULT,CW_USEDEFAULT,530,220,None,None,Some(instance),None).map_err(|e|e.to_string())?;
        let scale=(GetDpiForWindow(control) as f32/96.0).clamp(1.0,3.0);
        SetWindowPos(control,None,0,0,(530.0*scale) as i32,(220.0*scale) as i32,SWP_NOMOVE|SWP_NOACTIVATE|SWP_NOZORDER).map_err(|e|e.to_string())?;
        let mut logfont=LOGFONTW {lfHeight:-(16.0*scale) as i32,lfWeight:400,..Default::default()};
        for (target,value) in logfont.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {*target=value;}
        let font=CreateFontIndirectW(&logfont);
        let child=|name:PCWSTR,text:PCWSTR,style:WINDOW_STYLE,x:i32,y:i32,width:i32,height:i32,id:usize| {
            CreateWindowExW(WINDOW_EX_STYLE(0),name,text,WS_CHILD|WS_VISIBLE|style,
                (x as f32*scale) as i32,(y as f32*scale) as i32,(width as f32*scale) as i32,(height as f32*scale) as i32,
                Some(control),Some(HMENU(id as *mut core::ffi::c_void)),Some(instance),None)
                .inspect(|window|{SendMessageW(*window,WM_SETFONT,Some(WPARAM(font.0 as usize)),Some(LPARAM(1)));})
        };
        let combo=child(w!("COMBOBOX"),w!(""),WINDOW_STYLE(CBS_DROPDOWNLIST as u32)|WS_VSCROLL|WS_TABSTOP,16,15,376,250,GUIDES).map_err(|e|e.to_string())?;
        child(w!("BUTTON"),PCWSTR(wide(&crate::i18n::text("Refresh")).as_ptr()),WS_TABSTOP,404,15,98,28,REFRESH).map_err(|e|e.to_string())?;
        let status=child(w!("STATIC"),PCWSTR(wide(&crate::i18n::text("Waiting for the game and guide…")).as_ptr()),WINDOW_STYLE(0),16,58,486,58,0).map_err(|e|e.to_string())?;
        let pause=child(w!("BUTTON"),PCWSTR(wide(&crate::i18n::text("Pause")).as_ptr()),WS_TABSTOP,16,126,120,32,PAUSE).map_err(|e|e.to_string())?;
        child(w!("BUTTON"),PCWSTR(wide(&crate::i18n::text("Close overlay")).as_ptr()),WS_TABSTOP,146,126,140,32,QUIT).map_err(|e|e.to_string())?;
        SetWindowDisplayAffinity(overlay,WDA_EXCLUDEFROMCAPTURE).map_err(|e|e.to_string())?;
        let graphics=Graphics::new(overlay).map_err(|e|e.to_string())?;
        let mut ui=Box::new(Ui {overlay,status,pause,combo,shared,paused,exit,
            graphics:Some(graphics),last_revision:u64::MAX,last_guides:Vec::new(),last_selected:String::new(),
            last_status:String::new(),excluded:true,visible:false,_font:font,last_paused:false,wake:wake.clone()});
        let pointer=(&mut *ui) as *mut Ui as isize;
        SetWindowLongPtrW(overlay,GWLP_USERDATA,pointer);SetWindowLongPtrW(control,GWLP_USERDATA,pointer);
        SetTimer(Some(control),1,50,None);
        FOREGROUND_WAKE.with(|slot|*slot.borrow_mut()=Some(wake));
        let foreground=ForegroundEvents(SetWinEventHook(EVENT_SYSTEM_FOREGROUND,EVENT_SYSTEM_FOREGROUND,None,
            Some(foreground_changed),0,0,WINEVENT_OUTOFCONTEXT));
        if let Ok(mut state)=ui.shared.lock(){state.foreground_events = !foreground.0.0.is_null();}
        let mut tray=NOTIFYICONDATAW {cbSize:std::mem::size_of::<NOTIFYICONDATAW>() as u32,hWnd:control,uID:1,
            uFlags:NIF_MESSAGE|NIF_ICON|NIF_TIP,uCallbackMessage:TRAY,..Default::default()};
        let tip=wide("HOI4 Companion");tray.szTip[..tip.len()].copy_from_slice(&tip);
        tray.hIcon=LoadIconW(None,IDI_APPLICATION).unwrap_or_default();
        let tray_added=discreet && Shell_NotifyIconW(NIM_ADD,&tray).as_bool();
        if let Ok(mut s)=ui.shared.lock(){s.tray_controls=tray_added;}
        // Keep the legacy panel visible if notification-area registration fails.
        if !tray_added {let _=ShowWindow(control,SW_SHOWNOACTIVATE);}
        ready.send(Ok((control.0 as isize,overlay.0 as isize))).map_err(|e|e.to_string())?;
        let mut message=MSG::default();
        loop {
            let result=GetMessageW(&mut message,None,0,0).0;
            if result<=0 {break;}
            if !IsDialogMessageW(control,&message).as_bool() {
                let _=TranslateMessage(&message);DispatchMessageW(&message);
            }
        }
        if tray_added {let _=Shell_NotifyIconW(NIM_DELETE,&tray);}
        ui.hide();KillTimer(Some(control),1).ok();
        SetWindowLongPtrW(overlay,GWLP_USERDATA,0);SetWindowLongPtrW(control,GWLP_USERDATA,0);
        let _=DestroyWindow(overlay);let _=DestroyWindow(control);
        ui.graphics=None;
        let _=DeleteObject(font.into());
        UnregisterClassW(class.lpszClassName,Some(instance)).ok();
        Ok(())
    }
}

struct ComGuard;
impl Drop for ComGuard {fn drop(&mut self) {unsafe {CoUninitialize();}}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_evidence_renewals_do_not_require_an_immediate_ui_redraw() {
        let mut shared=Shared::default();
        let first=frame();
        assert!(shared.publish(first.clone()));
        let revision=shared.revision;
        let mut renewed=first;
        renewed.observed_at=Instant::now();
        assert!(!shared.publish(renewed), "Only a new visual needs an immediate UI message");
        assert_eq!(shared.revision,revision);
        let mut changed=frame();
        changed.marks[0].caption.push('!');
        assert!(shared.publish(changed));
        assert_eq!(shared.revision,revision+1);
    }
    #[test]
    fn repeated_metadata_updates_do_not_queue_render_messages() {
        let mut shared=Shared::default();let guides=vec![("one".into(),"Guide one".into())];
        assert!(shared.set_guides(guides.clone(),"one"));
        assert!(!shared.set_guides(guides.clone(),"one"));
        assert!(shared.set_guides(guides,"two"));
        assert!(shared.set_status("Ready".into()));
        assert!(!shared.set_status("Ready".into()));
        assert!(shared.set_status("Unavailable".into()));
        assert_eq!(shared.revision,0,"Metadata must never reset an admitted visual");
    }
    #[test]
    fn strokes_stay_inside_fractional_clipped_widgets_at_all_dpis() {
        for scale in [1.0,1.25,1.5,2.0,3.0] {
            for rect in [[0.0,0.0,8.0,8.0],[0.25,60.75,40.5,100.125],[770.0,580.0,800.0,600.0]] {
                for requested in [1.4*scale,1.8*scale,4.0*scale] {
                    let width=border_width(rect,requested);let path=border_path(rect,width,scale);
                    let [left,top,right,bottom]=rect;let p=path.rect;let epsilon=0.0001;
                    assert!((p.left-width/2.0-left).abs()<epsilon);
                    assert!((p.top-width/2.0-top).abs()<epsilon);
                    assert!((p.right+width/2.0-right).abs()<epsilon);
                    assert!((p.bottom+width/2.0-bottom).abs()<epsilon);
                    assert!(p.right>p.left && p.bottom>p.top);
                    assert!(path.radiusX<=(p.right-p.left)/2.0 && path.radiusY<=(p.bottom-p.top)/2.0);
                }
            }
        }
    }
    fn frame()->OverlayFrame {OverlayFrame {target:TargetWindow {hwnd:1,pid:1,x:0,y:0,width:800,height:600},
        marks:vec![Highlight {rect:[10.0,20.0,80.0,60.0],caption:"Ricerca".into(),active:false,choice:false,panel:"research".into()}],
        observed_at:Instant::now(),valid_for:Duration::from_secs(1)}}
    #[test]
    fn nonfinite_out_of_bounds_and_excessive_lifetimes_are_rejected() {
        assert!(validate_frame(&frame()).is_ok());
        let mut bad=frame();bad.marks[0].rect[0]=f32::NAN;assert!(validate_frame(&bad).is_err());
        let mut bad=frame();bad.marks[0].rect[2]=900.0;assert!(validate_frame(&bad).is_err());
        let mut bad=frame();bad.valid_for=Duration::from_secs(3);assert!(validate_frame(&bad).is_err());
    }
    #[test]
    fn fresh_evidence_renews_safety_deadline_without_invalidating_visuals() {
        let mut shared=Shared::default();
        let mut first=frame();first.observed_at=Instant::now()-Duration::from_millis(500);
        first.valid_for=Duration::from_millis(750);
        shared.publish(first.clone());let revision=shared.revision;
        let mut fresh=first;fresh.observed_at=Instant::now();
        shared.publish(fresh.clone());
        assert_eq!(shared.revision,revision,"Renewing evidence must not clear and present an identical layer");
        assert_eq!(shared.desired.as_ref().unwrap().observed_at,fresh.observed_at);
        assert_eq!(shared.desired.as_ref().unwrap().valid_for,Duration::from_millis(750));
        fresh.marks[0].rect[0]+=1.;shared.publish(fresh.clone());
        assert_eq!(shared.revision,revision+1);
        fresh.marks[0].caption="Focus".into();shared.publish(fresh.clone());
        assert_eq!(shared.revision,revision+2);
        fresh.target.x+=1;shared.publish(fresh);
        assert_eq!(shared.revision,revision+3);
        assert!(shared.invalidate());let hidden_revision=shared.revision;
        assert!(!shared.invalidate());assert_eq!(shared.revision,hidden_revision);
    }
    #[test]
    fn production_captions_stay_outside_rows_and_each_other_at_multiple_dpis() {
        for scale in [1.0,1.5,2.0] {
            let mut f=frame();f.target.width=(1280.0*scale) as u32;f.target.height=(900.0*scale) as u32;
            f.marks=(0..8).map(|i|Highlight {rect:[8.0*scale,(120.0+i as f32*80.0)*scale,508.0*scale,(190.0+i as f32*80.0)*scale],
                caption:"Tank · requested factories and configuration".into(),active:false,choice:false,panel:"production".into()}).collect();
            // The add-category cue can be close to the first row without its
            // caption covering any game controls or colliding with the row cue.
            f.marks.push(Highlight {rect:[100.0*scale,90.0*scale,170.0*scale,120.0*scale],caption:"Add tank".into(),active:false,choice:false,panel:"production".into()});
            let layout=caption_layout(&f,scale);
            assert!(layout.iter().all(Option::is_some));
            for (i,rect) in layout.iter().enumerate() {
                let r=rect.unwrap();assert!(r[0]>508.0*scale);assert!(r[2]-r[0]<=300.0*scale);assert_eq!(r[3]-r[1],20.0*scale);
                assert!(f.marks.iter().all(|m|!overlaps(r,m.rect,0.0)));
                assert!(layout[..i].iter().flatten().all(|other|!overlaps(r,*other,0.0)));
            }
        }
    }
    #[test]
    fn completed_production_keeps_border_without_caption_and_narrow_view_never_covers_controls() {
        let mut f=frame();f.marks[0].panel="production".into();f.marks[0].active=true;
        assert_eq!(caption_layout(&f,1.0),vec![None]);
        f.marks[0].active=false;f.marks[0].rect=[0.0,60.0,800.0,150.0];
        assert_eq!(caption_layout(&f,1.0),vec![None]);
    }
    #[test]
    fn tree_labels_use_external_space_or_omit_instead_of_covering_any_highlight() {
        let mut f=frame();f.marks[0].rect=[0.0,0.0,800.0,600.0];
        assert_eq!(caption_layout(&f,1.0),vec![None]);
        f.marks[0].rect=[10.0,0.0,80.0,40.0];
        let r=caption_layout(&f,1.0)[0].unwrap();
        assert!(!overlaps(r,f.marks[0].rect,0.0));assert!(r[0]>=0.0&&r[1]>=0.0&&r[2]<=800.0&&r[3]<=600.0);
    }
}
