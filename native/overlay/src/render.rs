//! Native external Direct2D overlay. No injected hooks, game input, or process-memory APIs.
//!
//! Rendering and native controls live on one dedicated STA/message-loop thread.
//! Callers only publish the latest frame, so a busy renderer cannot queue stale
//! highlights. The safety timer hides independently of recognition progress.
use std::collections::{HashMap, VecDeque};
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
use windows::Win32::Graphics::Gdi::{BitBlt, ClientToScreen, ScreenToClient, CreateCompatibleDC, CreateDIBSection,
    CreateFontIndirectW, DeleteDC, DeleteObject, GetDC, GetSysColorBrush, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, COLOR_WINDOW, DIB_RGB_COLORS, HFONT, LOGFONTW, SRCCOPY};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK,SetWinEventHook,UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey,UnregisterHotKey,SetCapture,ReleaseCapture,MOD_CONTROL,MOD_ALT,MOD_NOREPEAT};
use windows::Win32::UI::Shell::{Shell_NotifyIconW,NOTIFYICONDATAW,NIF_MESSAGE,NIF_ICON,NIF_TIP,NIM_ADD,NIM_DELETE};
use crate::wake::Wake;
use crate::card::{self,Button,Notice,Prefs};
use crate::palette::{self,Role};

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
const CARD_TOGGLE: usize = 205;
const CARD_HOTKEY: i32 = 1;
const MAX_MARKS: usize = 64;
/// Characters of a highlight caption drawn; longer captions are cut.
pub const MAX_CAPTION: usize = 150;
/// Compositor fade when the layer appears; never applied on disappearance.
const FADE_IN: Duration = Duration::from_millis(90);
/// Interval for re-asserting the topmost position without moving the window.
const TOPMOST_REFRESH: Duration = Duration::from_secs(1);

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
    /// What the highlight means, and so its color (see `palette`).
    pub role: Role,
    pub panel: String,
}

fn overlaps(a:[f32;4],b:[f32;4],gap:f32)->bool {
    a[0]<b[2]+gap && a[2]+gap>b[0] && a[1]<b[3]+gap && a[3]+gap>b[1]
}

/// Place captions outside all highlighted controls. Production uses a compact
/// side column, rather than covering the previous line's name/factory counters.
/// When no safe space remains, keep the border and omit its caption.
#[cfg(test)]
fn caption_layout(frame:&OverlayFrame,scale:f32)->Vec<Option<[f32;4]>> {caption_layout_avoiding(frame,scale,&[])}
#[cfg(test)]
fn caption_layout_avoiding(frame:&OverlayFrame,scale:f32,avoid:&[[f32;4]])->Vec<Option<[f32;4]>> {
    caption_slots(frame,scale,avoid,&HashMap::new()).into_iter().map(|slot|slot.map(|(_,rect)|rect)).collect()
}
/// Caption identity kept across frames, independent of the control's position.
fn caption_key(mark:&Highlight)->String {format!("{}{}",mark.panel,mark.caption)}
/// Like the first-fit placement, but a caption keeps the candidate side it used
/// in the previous frame while that side is still free. Without this a caption
/// jumps between above/right/below as the player pans or another label moves.
fn caption_slots(frame:&OverlayFrame,scale:f32,avoid:&[[f32;4]],preferred:&HashMap<String,usize>)->Vec<Option<(usize,[f32;4])>> {
    let width=frame.target.width as f32;let height=frame.target.height as f32;
    let gap=6.0*scale;
    let production_right=frame.marks.iter().filter(|m|m.panel=="production")
        .map(|m|m.rect[2]).fold(0.0_f32,f32::max);
    let mut placed:Vec<[f32;4]>=avoid.to_vec();let mut result=Vec::new();
    for mark in &frame.marks {
        let production=mark.panel=="production";
        if mark.caption.is_empty() || production&&mark.role==Role::Active {result.push(None);continue;}
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
        let free=|r:&[f32;4]| r[0]>=0.0&&r[1]>=0.0&&r[2]<=width&&r[3]<=height&&
            !frame.marks.iter().any(|other|overlaps(*r,other.rect,gap/2.0))&&
            !placed.iter().any(|other|overlaps(*r,*other,gap/2.0));
        let selected=preferred.get(&caption_key(mark)).and_then(|i|candidates.get(*i).filter(|r|free(r)).map(|r|(*i,*r)))
            .or_else(||candidates.iter().enumerate().find(|(_,r)|free(r)).map(|(i,r)|(i,*r)));
        if let Some((_,rect))=selected {placed.push(rect);}
        result.push(selected);
    }
    result
}

#[derive(Clone, Debug)]
pub struct OverlayFrame {
    pub target: TargetWindow,
    pub marks: Vec<Highlight>,
    /// The guide step to do now, shown as a panel on the right of the game
    /// window (under the theater bar), with or without highlighted controls.
    pub card: Option<StepCard>,
    /// Witness time of the highlights; with `valid_for` it bounds only them.
    pub observed_at: Instant,
    pub valid_for: Duration,
    /// Last time the main loop confirmed the card for this window. The card
    /// has its own lifetime, so expired highlights never take it down.
    pub card_observed_at: Instant,
}
/// Card lifetime if the main loop stops refreshing it (stall protection).
pub const CARD_VALID_FOR: Duration = Duration::from_millis(1500);
/// The layers still within their lifetimes, or None when nothing may be drawn.
fn visible_frame(frame:&OverlayFrame,now:Instant)->Option<OverlayFrame> {
    let marks_live=!frame.marks.is_empty()&&now.saturating_duration_since(frame.observed_at)<=frame.valid_for;
    let card_live=frame.card.is_some()&&now.saturating_duration_since(frame.card_observed_at)<=CARD_VALID_FOR;
    if !marks_live&&!card_live {return None;}
    let mut shown=frame.clone();
    if !marks_live {shown.marks.clear();}
    if !card_live {shown.card=None;}
    Some(shown)
}

/// Already localized card text; guide text stays as authored.
/// One quick-reference section of the guide (backend `guide_sections`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RefSection {pub label:String, pub rows:Vec<RefRow>}
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RefRow {pub id:String, pub text:String, pub heading:bool}
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StepCard {
    pub header: String,
    /// Guided line (UX R9): "Category · When · Verification", as in the suite.
    pub status_line: String,
    /// What follows: the next step and the nearest waiting one.
    pub upcoming: String,
    pub heading: String,
    /// The author's condition ("When: At 24% paranoia"), under the heading.
    pub timing: String,
    pub note: String,
    pub body: String,
    pub badge: String,
    /// Step identity for the "Done" button; empty when unknown.
    pub key: String,
    pub guide_id: String,
    /// The step accepts a manual confirmation (as the suite's checkbox).
    pub checkable: bool,
    /// Design/template reference images, as on the walkthrough page; each is
    /// drawn once its picture is decoded, with its name above it.
    pub images: Vec<crate::card_images::CardImage>,
    /// The guide's current production plan, one localized line per product:
    /// target and live requested/assigned factories.
    pub production: String,
    /// Army setup, war plans and events, opened from tags on the card.
    pub references: Vec<RefSection>,
    /// Checklist run the reference marks belong to ("guide:run").
    pub references_scope: String,
    /// The ambiguous event answer the popup shows now, which the player can
    /// confirm as the guide's answer ("I chose this").
    pub answer: Option<Answer>,
    /// The step's event answer was confirmed by the player earlier.
    pub confirmed_by_user: bool,
    /// The backend offers "Set aside" for this step.
    pub can_set_aside: bool,
    /// Steps the player set aside (key, title), each with a "Resume" button.
    pub set_aside: Vec<(String,String)>,
}
/// An ambiguous event answer as the backend proposes it: the guide line
/// (`key` and its `quote`) and the visible option it may mean.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Answer {pub key:String, pub quote:String, pub event_id:String, pub option_id:String}
/// Card frame on the right edge, below the game's top bar and theater selector.
fn card_origin(width:f32,height:f32,scale:f32)->(f32,f32,f32) {
    let card_width=(360.0*scale).min(width*0.42);
    let top=(height*0.13).max(96.0*scale).min(height*0.5);
    (width-card_width-16.0*scale,top,card_width)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlAction {
    SelectGuide(String),
    RefreshGuides,
    OpenSuite,
    /// "Done" or "Undo" on the step card: a manual confirmation of that step.
    Check {guide_id:String,key:String,checked:bool},
    /// "I chose this" on the card: confirm the visible answer as the guide's.
    ConfirmAnswer {guide_id:String,key:String,quote:String,event_id:String,option_id:String},
    /// "Undo" after "I chose this": restore what the confirmation replaced.
    RestoreAnswer {guide_id:String,key:String,quote:String,previous:serde_json::Value,previous_checked:bool},
    /// "Set aside" (aside) or "Resume" on the card: a step of the active run.
    SetAside {guide_id:String,key:String,aside:bool},
}
/// The request the card's Undo button sends.
pub fn undo_action(undo:&card::Undo)->ControlAction {
    match undo {
        card::Undo::Check {guide_id,key}=>ControlAction::Check {guide_id:guide_id.clone(),key:key.clone(),checked:false},
        card::Undo::Answer {guide_id,key,quote,previous,previous_checked}=>ControlAction::RestoreAnswer {
            guide_id:guide_id.clone(),key:key.clone(),quote:quote.clone(),previous:previous.clone(),previous_checked:*previous_checked},
        card::Undo::Aside {guide_id,key,aside}=>ControlAction::SetAside {guide_id:guide_id.clone(),key:key.clone(),aside:!*aside},
    }
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
    /// Result of the last "Done" click, shown on the card.
    notice:Option<Notice>,
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
            previous.target!=frame.target || previous.marks!=frame.marks || previous.card!=frame.card);
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
    pub fn new(discreet:bool,managed:bool,prefs_path:std::path::PathBuf) -> Result<Self, String> {
        let shared=Arc::new(Mutex::new(Shared { excluded:true, ..Shared::default() }));
        let paused=Arc::new(AtomicBool::new(false));
        let exit=Arc::new(AtomicBool::new(false));
        let wake=Arc::new(Wake::default());
        let (ready, receiver)=mpsc::sync_channel(1);
        let (s,p,e)=(shared.clone(),paused.clone(),exit.clone());
        let events=wake.clone();
        let thread=thread::Builder::new().name("overlay-window".into()).spawn(move || {
            let result=unsafe { run(s,p,e.clone(),events,discreet,managed,prefs_path,&ready) };
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
    pub fn set_notice(&self,notice:Option<Notice>) {
        if let Ok(mut s)=self.shared.lock() {s.notice=notice;}
        self.update();
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
        || frame.valid_for.is_zero() || frame.valid_for>Duration::from_secs(2)
        || frame.card.as_ref().is_some_and(|c|[&c.header,&c.heading,&c.note,&c.body,&c.badge].iter().any(|t|t.chars().count()>2000)) {
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

/// Brushes, all from `palette`: one stroke and one fill per role.
struct Paint {
    strokes:[ID2D1SolidColorBrush;5],
    fills:[ID2D1SolidColorBrush;5],
    /// Border of an ambiguous answer's highlight.
    dashed:ID2D1StrokeStyle,
    shadow:ID2D1SolidColorBrush,
    label_bg:ID2D1SolidColorBrush,
    card_bg:ID2D1SolidColorBrush,
    card_border:ID2D1SolidColorBrush,
    text:ID2D1SolidColorBrush,
    muted:ID2D1SolidColorBrush,
    /// Card checklist emphasis (see `card::Hue`): still to do.
    todo:ID2D1SolidColorBrush,
}
impl Paint {
    unsafe fn new(context:&ID2D1DeviceContext)->windows::core::Result<Self> {
        unsafe {
            let brush=|color:D2D1_COLOR_F|context.CreateSolidColorBrush(&color,None);
            let [a,b,c,d,e]=Role::ALL;
            let stroke=|role:Role|brush(palette::color(role));
            let fill=|role:Role|brush(palette::with_alpha(palette::color(role),palette::FILL_ALPHA));
            let dashed=context.GetFactory()?.CreateStrokeStyle(&D2D1_STROKE_STYLE_PROPERTIES {
                dashStyle:D2D1_DASH_STYLE_DASH,dashCap:D2D1_CAP_STYLE_FLAT,..Default::default()},None)?;
            Ok(Self {strokes:[stroke(a)?,stroke(b)?,stroke(c)?,stroke(d)?,stroke(e)?],
                fills:[fill(a)?,fill(b)?,fill(c)?,fill(d)?,fill(e)?],dashed,
                shadow:brush(palette::SHADOW)?,
                label_bg:brush(palette::LABEL_BACKGROUND)?,
                card_bg:brush(palette::CARD_BACKGROUND)?,
                card_border:brush(palette::with_alpha(palette::color(Role::Next),palette::CARD_BORDER_ALPHA))?,
                text:brush(palette::TEXT)?,
                muted:brush(palette::MUTED)?,
                todo:brush(palette::TODO)?})
        }
    }
    fn stroke(&self,role:Role)->&ID2D1SolidColorBrush {&self.strokes[role.index()]}
    fn fill(&self,role:Role)->&ID2D1SolidColorBrush {&self.fills[role.index()]}
}

struct Graphics {
    context: ID2D1DeviceContext,
    swap: IDXGISwapChain1,
    composition: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    /// Layer opacity, animated by the compositor for a short fade-in only.
    effect: Option<IDCompositionEffectGroup>,
    caption_sides: HashMap<String,usize>,
    write: IDWriteFactory,
    paint: Paint,
    formats: Option<(f32,IDWriteTextFormat,IDWriteTextFormat)>,
    card_formats: Option<(f32,[IDWriteTextFormat;5])>,
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
            visual.SetContent(&swap)?;target.SetRoot(&visual)?;
            // Optional: without an effect group the layer simply appears at once.
            let effect=composition.CreateEffectGroup().ok().filter(|effect|
                effect.SetOpacity2(1.0).is_ok()&&visual.SetEffect(effect).is_ok());
            composition.Commit()?;
            let write:IDWriteFactory=DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let paint=Paint::new(&context)?;
            Ok(Self {context,swap,composition,_target:target,_visual:visual,effect,caption_sides:HashMap::new(),write,paint,formats:None,card_formats:None,bitmap:None,size:(0,0)})
        }
    }
    /// Fade the whole layer in when it becomes visible. Disappearance stays
    /// instantaneous so an expired highlight is never kept on screen.
    unsafe fn fade_in(&self) {
        unsafe {
            let Some(effect)=self.effect.as_ref() else {return;};
            let animated=(||{
                let animation=self.composition.CreateAnimation()?;
                animation.AddCubic(0.0,0.0,(1.0/FADE_IN.as_secs_f64()) as f32,0.0,0.0)?;
                animation.End(FADE_IN.as_secs_f64(),1.0)?;
                effect.SetOpacity(&animation)?;self.composition.Commit()
            })();
            if animated.is_err() {let _=effect.SetOpacity2(1.0);let _=self.composition.Commit();}
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
    unsafe fn ensure_card_formats(&mut self,scale:f32)->windows::core::Result<()> {
        unsafe {
            if self.card_formats.as_ref().is_some_and(|(previous,_)|*previous==scale) {return Ok(());}
            let make=|size:f32,weight:DWRITE_FONT_WEIGHT|self.write.CreateTextFormat(w!("Segoe UI"),None,weight,
                DWRITE_FONT_STYLE_NORMAL,DWRITE_FONT_STRETCH_NORMAL,size*scale,w!("it-IT"));
            let formats=[make(11.0,DWRITE_FONT_WEIGHT_SEMI_BOLD)?,make(16.0,DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
                make(12.0,DWRITE_FONT_WEIGHT_NORMAL)?,make(13.0,DWRITE_FONT_WEIGHT_NORMAL)?,make(12.0,DWRITE_FONT_WEIGHT_SEMI_BOLD)?];
            for format in &formats[..4] {format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;}
            formats[4].SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            formats[4].SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            formats[4].SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            self.card_formats=Some((scale,formats));Ok(())
        }
    }
    unsafe fn measure(&self,text:&[u16],format:usize,width:f32,height:f32)->windows::core::Result<(f32,f32)> {
        unsafe {
            let layout=self.write.CreateTextLayout(text,&self.card_formats.as_ref().unwrap().1[format],width.max(1.0),height.max(1.0))?;
            let mut metrics=DWRITE_TEXT_METRICS::default();layout.GetMetrics(&mut metrics)?;
            Ok((metrics.widthIncludingTrailingWhitespace,metrics.height))
        }
    }
    /// Draw the step card into this (card-sized) surface. Returns the card's
    /// size and its header buttons in card coordinates. Text that does not fit
    /// in `max_height` is clipped.
    unsafe fn draw_card(&mut self,card:&StepCard,collapsed:bool,notice:Option<&Notice>,width:f32,max_height:f32,scale:f32,
        open:Option<usize>,marks:&crate::card::Marks,production_open:bool)
        ->windows::core::Result<((f32,f32),Vec<(Button,[f32;4])>)> {
        unsafe {
            self.ensure_card_formats(scale)?;
            let pad=14.0*scale;let inner=(width-2.0*pad).max(40.0);let row=24.0*scale;let gap=6.0*scale;
            // Header buttons, right to left: hide, collapse, then "I chose
            // this" for an ambiguous answer or "Done" for a checkable step.
            let top=10.0*scale;
            let mut sized:Vec<(Button,String,f32)>=Vec::new();
            for button in card::header_buttons(card.answer.is_some(),card.answer.as_ref().is_some_and(|a|a.key==card.key),card.checkable && !card.key.is_empty()) {
                let label=match (button,card::header_label(button,card.confirmed_by_user)) {
                    (_,Some(label))=>format!("✓ {}",crate::i18n::text(label)),
                    (Button::Collapse,_)=>if collapsed {"+".into()}else{"–".into()},
                    _=>"×".into(),
                };
                let w=if matches!(button,Button::Hide|Button::Collapse) {row} else {
                    let wide:Vec<u16>=label.encode_utf16().collect();
                    self.measure(&wide,4,400.0*scale,row)?.0+18.0*scale
                };
                sized.push((button,label,w));
            }
            let (placed,right)=card::header_layout(&sized.iter().map(|(b,_,w)|(*b,*w)).collect::<Vec<_>>(),width-pad+4.0*scale,top,row,gap);
            let mut buttons:Vec<(Button,String,[f32;4])>=placed.into_iter().zip(sized).map(|((b,r),(_,label,_))|(b,label,r)).collect();
            let mut lines:Vec<(Vec<u16>,usize,D2D_RECT_F,u8)>=Vec::new();
            let header:Vec<u16>=card.header.chars().filter(|c|!c.is_control()).collect::<String>().encode_utf16().collect();
            let header_width=(right+gap-8.0*scale-pad).max(40.0*scale);
            let header_height=if header.is_empty() {row} else {self.measure(&header,0,header_width,max_height)?.1.max(row)};
            if !header.is_empty() {
                lines.push((header,0,D2D_RECT_F {left:pad,top,right:pad+header_width,bottom:top+header_height},0));
            }
            let mut y=top+header_height+8.0*scale;
            let limit=max_height-10.0*scale;
            let mut blocks:Vec<(String,usize,u8)>=Vec::new();
            let open=open.filter(|i|*i<card.references.len()&&!collapsed);
            if !collapsed {
                blocks.push((card.status_line.clone(),2,3));
                blocks.push((card.heading.clone(),1,1));
                blocks.push((card.timing.clone(),3,0));
                // An open reference section replaces the step details below.
                if open.is_none() {
                    // An ambiguous answer's badge takes the ambiguous color (tone 5).
                    for (text,format,tone) in [(&card.note,2usize,2u8),(&card.body,3,4),(&card.upcoming,2,2),(&card.badge,2,if card.answer.is_some() {5} else {3})] {
                        blocks.push((text.clone(),format,tone));
                    }
                }
            }
            // One wrapped text block at `y`; None when empty or out of room.
            let block=|this:&Self,text:&str,format:usize,tone:u8,y:&mut f32,lines:&mut Vec<(Vec<u16>,usize,D2D_RECT_F,u8)>|
                ->windows::core::Result<Option<D2D_RECT_F>> {
                let text:String=text.chars().filter(|c|*c=='\n'||!c.is_control()).collect();
                if text.trim().is_empty() {return Ok(None);}
                let wide:Vec<u16>=text.encode_utf16().collect();
                let available=(limit-*y).max(0.0);
                if available<8.0*scale {return Ok(None);}
                let used=this.measure(&wide,format,inner,available)?.1.min(available);
                let rect=D2D_RECT_F {left:pad,top:*y,right:pad+inner,bottom:*y+used};
                lines.push((wide,format,rect,tone));*y+=used+8.0*scale;
                Ok(Some(rect))
            };
            let mut blocks=blocks.into_iter();
            // The guided line, the heading and its condition come before the quick-reference tags.
            for (text,format,tone) in blocks.by_ref().take(3) {block(self,&text,format,tone,&mut y,&mut lines)?;}
            // Quick-reference tags: a click opens the section (never a hover).
            if !collapsed && !card.references.is_empty() && y+row<=limit {
                let mut x=pad;
                for (i,section) in card.references.iter().enumerate().take(8) {
                    let left=section.rows.iter().filter(|r|!r.heading&&!marks.is_done(&card.references_scope,&r.id)).count();
                    let label=format!("{}{} {}",if open==Some(i){"▾ "}else{""},crate::i18n::text(&section.label),if left==0 {"✓".to_owned()}else{left.to_string()});
                    let wide:Vec<u16>=label.encode_utf16().collect();
                    let w=self.measure(&wide,4,400.0*scale,row)?.0+18.0*scale;
                    if x+w>pad+inner && x>pad {x=pad;y+=row+gap;}
                    if y+row>limit {break;}
                    buttons.push((Button::Tag(i as u8),label,[x,y,x+w,y+row]));x+=w+gap;
                }
                y+=row+8.0*scale;
            }
            if let Some(index)=open {
                let section=&card.references[index];
                let boxed=18.0*scale;
                for (j,r) in section.rows.iter().enumerate() {
                    if y>=limit-8.0*scale {break;}
                    if r.heading {block(self,&r.text,0,0,&mut y,&mut lines)?;continue;}
                    let done=marks.is_done(&card.references_scope,&r.id);
                    let text:String=r.text.chars().filter(|c|!c.is_control()).collect();
                    let wide:Vec<u16>=text.encode_utf16().collect();
                    let text_left=pad+boxed+8.0*scale;
                    let available=(limit-y).max(0.0);
                    let used=self.measure(&wide,3,(pad+inner-text_left).max(40.0),available)?.1.min(available).max(boxed);
                    lines.push((wide,3,D2D_RECT_F {left:text_left,top:y,right:pad+inner,bottom:y+used},if done {2} else {1}));
                    buttons.push((Button::Item(j as u16),if done {"✓".into()} else {String::new()},[pad,y+1.0*scale,pad+boxed,y+1.0*scale+boxed]));
                    y+=used+6.0*scale;
                }
                y+=4.0*scale;
            }
            for (text,format,tone) in blocks {block(self,&text,format,tone,&mut y,&mut lines)?;}
            // "Set aside" for this step, then the steps set aside, each one
            // click from coming back ("Resume"). Never a completion by itself.
            if !collapsed && open.is_none() && !card.guide_id.is_empty() {
                let fit=|this:&Self,label:&str|->windows::core::Result<f32> {
                    let wide:Vec<u16>=label.encode_utf16().collect();
                    Ok((this.measure(&wide,4,400.0*scale,row)?.0+18.0*scale).min(inner))
                };
                if card.can_set_aside && !card.key.is_empty() && y+row<=limit {
                    let label=format!("⏸ {}",crate::i18n::text("Set aside for later"));
                    let w=fit(self,&label)?;
                    buttons.push((Button::SetAside,label,[pad,y,pad+w,y+row]));y+=row+8.0*scale;
                }
                if !card.set_aside.is_empty() && block(self,&crate::i18n::text("Set aside steps"),2,2,&mut y,&mut lines)?.is_some() {
                    for (i,(_,title)) in card.set_aside.iter().enumerate().take(6) {
                        if y+row>limit {break;}
                        let short:String=title.chars().take(48).collect();
                        let label=format!("↩ {} · {}",crate::i18n::text("Resume"),if short.len()<title.len() {format!("{short}…")} else {short});
                        let w=fit(self,&label)?;
                        buttons.push((Button::Resume(i as u8),label,[pad,y,pad+w,y+row]));y+=row+gap;
                    }
                    y+=8.0*scale-gap;
                }
            }
            // The production plan stays behind its button until the player opens it.
            if !collapsed && open.is_none() && !card.production.trim().is_empty() && y+row<=limit {
                let count=card.production.lines().filter(|l|!l.trim().is_empty()).count();
                let label=format!("{} {} · {}",if production_open {"▾"} else {"▸"},crate::i18n::text("Production"),count);
                let wide:Vec<u16>=label.encode_utf16().collect();
                let w=(self.measure(&wide,4,400.0*scale,row)?.0+18.0*scale).min(inner);
                buttons.push((Button::Production,label,[pad,y,pad+w,y+row]));y+=row+8.0*scale;
                if production_open {block(self,&card.production,3,4,&mut y,&mut lines)?;}
            }
            // Reference pictures (as on the walkthrough page) under the text,
            // each with its name. Until a picture is decoded, or if it fails,
            // only the name is shown; a picture that does not fit is left out.
            let mut pictures:Vec<(std::sync::Arc<crate::card_images::Thumb>,D2D_RECT_F)>=Vec::new();
            if !collapsed && open.is_none() {
                for image in &card.images {
                    let label=crate::i18n::text("Design reference");
                    let label=if image.name.is_empty() {label} else {format!("{label} · {}",image.name)};
                    if block(self,&label,2,2,&mut y,&mut lines)?.is_none() {break;}
                    let Some(thumb)=image.thumb.as_ref() else {continue;};
                    let (tw,th)=(thumb.width as f32,thumb.height as f32);
                    let (mut w,cap)=(inner.min(tw*scale),(max_height/3.0).max(60.0*scale));
                    let mut h=w*th/tw;
                    if h>cap {h=cap;w=h*tw/th;}
                    if y+h>limit {continue;}
                    pictures.push((thumb.clone(),D2D_RECT_F {left:pad,top:y,right:pad+w,bottom:y+h}));y+=h+8.0*scale;
                }
            }
            let notice_rect=match notice {Some(notice)=>block(self,&notice.text,2,3,&mut y,&mut lines)?,None=>None};
            if let (Some(rect),Some(notice))=(notice_rect,notice) && notice.undo.is_some() && y+row<=limit {
                let label=crate::i18n::text("Undo");
                let wide:Vec<u16>=label.encode_utf16().collect();
                let w=self.measure(&wide,4,400.0*scale,row)?.0+18.0*scale;
                buttons.push((Button::Undo,label,[rect.left,y,rect.left+w,y+row]));y+=row+8.0*scale;
            }
            let height=(y-8.0*scale+10.0*scale).min(max_height).max(top+row+10.0*scale).ceil();
            let width=width.ceil();
            self.size(width as u32,height as u32)?;
            // A picture whose bitmap cannot be created is skipped, never the card.
            let bitmaps:Vec<(ID2D1Bitmap1,D2D_RECT_F)>=pictures.iter().filter_map(|(thumb,rect)|{
                let properties=D2D1_BITMAP_PROPERTIES1 {pixelFormat:D2D1_PIXEL_FORMAT {
                    format:DXGI_FORMAT_B8G8R8A8_UNORM,alphaMode:D2D1_ALPHA_MODE_PREMULTIPLIED},
                    dpiX:96.0,dpiY:96.0,bitmapOptions:D2D1_BITMAP_OPTIONS_NONE,..Default::default()};
                self.context.CreateBitmap(D2D_SIZE_U {width:thumb.width,height:thumb.height},
                    Some(thumb.bgra.as_ptr().cast()),thumb.width*4,&properties).ok().map(|bitmap|(bitmap,*rect))
            }).collect();
            let formats=&self.card_formats.as_ref().unwrap().1;
            self.context.BeginDraw();self.context.Clear(Some(&palette::TRANSPARENT));
            let panel=D2D1_ROUNDED_RECT {rect:D2D_RECT_F {left:0.5,top:0.5,right:width-0.5,bottom:height-0.5},radiusX:6.0*scale,radiusY:6.0*scale};
            self.context.FillRoundedRectangle(&panel,&self.paint.card_bg);
            self.context.DrawRoundedRectangle(&panel,&self.paint.card_border,1.0*scale,None);
            for (bitmap,rect) in &bitmaps {
                self.context.DrawBitmap(bitmap,Some(rect),1.0,D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,None,None);
                self.context.DrawRectangle(rect,&self.paint.card_border,1.0*scale,None);
            }
            for (text,format,rect,tone) in &lines {
                let brush=match tone {0=>self.paint.stroke(Role::Next),1|4=>&self.paint.text,2=>&self.paint.muted,5=>self.paint.stroke(Role::Ambiguous),_=>self.paint.stroke(Role::Choice)};
                if *tone!=4 {
                    self.context.DrawText(text,&formats[*format],rect,brush,D2D1_DRAW_TEXT_OPTIONS_CLIP,DWRITE_MEASURING_MODE_NATURAL);
                    continue;
                }
                // Checklist text: marks, counts and states in color (card::spans).
                let layout=self.write.CreateTextLayout(text,&formats[*format],(rect.right-rect.left).max(1.0),(rect.bottom-rect.top).max(1.0))?;
                for (start,length,hue) in crate::card::spans(&String::from_utf16_lossy(text)) {
                    let range=DWRITE_TEXT_RANGE {startPosition:start,length};
                    let effect=match hue {
                        crate::card::Hue::Done=>Some(self.paint.stroke(Role::Done)),crate::card::Hue::Todo=>Some(&self.paint.todo),
                        crate::card::Hue::Queued=>Some(self.paint.stroke(Role::Active)),crate::card::Hue::Unknown=>Some(&self.paint.muted),
                        crate::card::Hue::Strong=>None,
                    };
                    if let Some(effect)=effect {layout.SetDrawingEffect(&effect.cast::<windows::core::IUnknown>()?,range)?;}
                    if hue!=crate::card::Hue::Unknown {layout.SetFontWeight(DWRITE_FONT_WEIGHT_SEMI_BOLD,range)?;}
                }
                self.context.DrawTextLayout(windows_numerics::Vector2 {X:rect.left,Y:rect.top},&layout,brush,D2D1_DRAW_TEXT_OPTIONS_CLIP);
            }
            for (button,label,[left,top,right,bottom]) in &buttons {
                let rect=D2D_RECT_F {left:*left,top:*top,right:*right,bottom:*bottom};
                let shape=D2D1_ROUNDED_RECT {rect,radiusX:4.0*scale,radiusY:4.0*scale};
                self.context.FillRoundedRectangle(&shape,&self.paint.label_bg);
                let accent=if matches!(button,Button::ConfirmAnswer) {Some(self.paint.stroke(Role::Ambiguous))}
                    else if matches!(button,Button::Done|Button::Undo|Button::Resume(_)) {Some(self.paint.stroke(Role::Active))} else {None};
                let border=if let Some(accent)=accent {accent}
                    else if matches!(button,Button::Tag(i) if open==Some(*i as usize)) || matches!(button,Button::Production if production_open) {self.paint.stroke(Role::Active)}
                    else if matches!(button,Button::Item(_)) {&self.paint.text}else{&self.paint.muted};
                self.context.DrawRoundedRectangle(&shape,border,1.0*scale,None);
                let text:Vec<u16>=label.encode_utf16().collect();
                let brush=if let Some(accent)=accent {accent}else if matches!(button,Button::Item(_)) {self.paint.stroke(Role::Done)}else{&self.paint.text};
                self.context.DrawText(&text,&formats[4],&rect,brush,D2D1_DRAW_TEXT_OPTIONS_CLIP,DWRITE_MEASURING_MODE_NATURAL);
            }
            self.context.EndDraw(None,None)?;self.swap.Present(0,DXGI_PRESENT(0)).ok()?;
            Ok(((width,height),buttons.into_iter().map(|(b,_,r)|(b,r)).collect()))
        }
    }
    /// Highlights only; the card has its own window. `avoid` is the card's
    /// rectangle in client coordinates, kept free of captions.
    unsafe fn draw(&mut self,frame:&OverlayFrame,avoid:&[[f32;4]])->windows::core::Result<()> {
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
            let slots=caption_slots(frame,scale,avoid,&self.caption_sides);
            self.caption_sides=frame.marks.iter().zip(&slots).filter_map(|(mark,slot)|slot.map(|(i,_)|(caption_key(mark),i))).collect();
            let captions:Vec<Option<[f32;4]>>=slots.into_iter().map(|slot|slot.map(|(_,rect)|rect)).collect();
            self.context.BeginDraw();self.context.Clear(Some(&palette::TRANSPARENT));
            let result={
                for (index,mark) in frame.marks.iter().enumerate() {
                    let stroke=self.paint.stroke(mark.role);let fill=self.paint.fill(mark.role);
                    // An ambiguous answer is outlined with dashes: a guess to confirm.
                    let style=(mark.role==Role::Ambiguous).then_some(&self.paint.dashed);
                    let width=border_width(mark.rect,if mark.panel=="production" {1.4}else{1.8}*scale);
                    let round=border_path(mark.rect,width,scale);
                    if mark.panel!="production" {
                        let shadow_width=border_width(mark.rect,4.0*scale);
                        self.context.DrawRoundedRectangle(&border_path(mark.rect,shadow_width,scale),&self.paint.shadow,shadow_width,None);
                        self.context.FillRoundedRectangle(&round,fill);
                    }
                    self.context.DrawRoundedRectangle(&round,stroke,width,style);
                    let caption:Vec<u16>=mark.caption.chars().filter(|c|!c.is_control()).take(MAX_CAPTION).collect::<String>().encode_utf16().collect();
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
    /// The step card's own window: it takes clicks (buttons, drag) but never
    /// activates, so HOI4 keeps the foreground and receives no input from it.
    card:HWND,
    status:HWND,
    pause:HWND,
    combo:HWND,
    shared:Arc<Mutex<Shared>>,
    paused:Arc<AtomicBool>,
    exit:Arc<AtomicBool>,
    graphics:Option<Graphics>,
    card_graphics:Option<Graphics>,
    last_revision:u64,
    last_guides:Vec<(String,String)>,
    last_selected:String,
    last_status:String,
    excluded:bool,
    visible:bool,
    /// Card rectangle the highlights were last drawn around.
    drawn_avoid:Option<[f32;4]>,
    /// Last window placement, so SetWindowPos runs only when it changes.
    placed:Option<[i32;4]>,
    topmost_at:Instant,
    card_visible:bool,
    card_placed:Option<[i32;4]>,
    /// Content last drawn on the card; a redraw happens only when it changes.
    card_state:Option<CardState>,
    card_size:(f32,f32),
    card_buttons:Vec<(Button,[f32;4])>,
    /// Card rectangle in the game's client coordinates, while shown.
    card_rect:Option<[f32;4]>,
    card_target:Option<TargetWindow>,
    card_content:Option<StepCard>,
    notice:Option<Notice>,
    prefs:Prefs,
    prefs_path:std::path::PathBuf,
    /// The quick-reference section open on the card, and the ticked rows.
    open_ref:Option<usize>,
    marks:crate::card::Marks,
    marks_path:std::path::PathBuf,
    drag:Option<Drag>,
    pressed:Option<Button>,
    _font:HFONT,
    last_paused:bool,
    wake:Arc<Wake>,
}

#[derive(Clone,PartialEq)]
struct CardState {card:StepCard,collapsed:bool,notice:Option<(String,bool)>,scale:u32,client:(u32,u32)}
/// A drag of the card: cursor offset inside the card when the button went down.
struct Drag {grab:(i32,i32),moved:bool}

fn window_scale(target:&TargetWindow)->f32 {
    unsafe {(GetDpiForWindow(hwnd(target.hwnd)) as f32/96.0).clamp(1.0,3.0)}
}

impl Ui {
    unsafe fn update(&mut self,redraw:bool) {
        unsafe {
            let Ok(shared)=self.shared.lock() else {self.hide();return;};
            let frame=shared.desired.clone();let revision=shared.revision;
            let guides=shared.guides.clone();let selected=shared.selected.clone();let status=shared.status.clone();let excluded=shared.excluded;
            let notice=shared.notice.clone().filter(|n|n.live(Instant::now()));
            drop(shared);
            self.notice=notice;
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
                let affinity=if excluded {WDA_EXCLUDEFROMCAPTURE}else{WDA_NONE};
                if SetWindowDisplayAffinity(self.overlay,affinity).is_err() || SetWindowDisplayAffinity(self.card,affinity).is_err() {
                    self.hide();self.exit.store(true,Ordering::Release);PostQuitMessage(1);return;
                }
                self.excluded=excluded;
            }
            let Some(frame)=frame else {self.hide();return;};
            if self.paused.load(Ordering::Acquire) || !target_matches(&frame.target) {self.hide();return;}
            // Each layer has its own lifetime: expired highlights leave the card.
            let Some(shown)=visible_frame(&frame,Instant::now()) else {self.hide();return;};
            // The card first: its rectangle is the area captions must avoid.
            let avoid=self.update_card(&frame.target,shown.card.as_ref());
            if shown.marks.is_empty() {self.hide_overlay();}
            else if redraw || revision!=self.last_revision || !self.visible || avoid!=self.drawn_avoid {
                match self.present(&frame,&shown,avoid) {
                    Ok(true)=>{self.drawn_avoid=avoid;self.last_revision=revision;}
                    Ok(false)=>{self.hide_overlay();}
                    Err(error)=>{
                        self.hide_overlay();let message=wide(&crate::i18n::format("Rendering suspended: {0}",&[&error.to_string()]));
                        let _=SetWindowTextW(self.status,PCWSTR(message.as_ptr()));
                    }
                }
            }else if self.topmost_at.elapsed()>=TOPMOST_REFRESH {
                let _=SetWindowPos(self.overlay,Some(HWND_TOPMOST),0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE);
                self.raise_card();
                self.topmost_at=Instant::now();
            }
        }
    }
    /// Keep the card above the click-through highlight layer.
    unsafe fn raise_card(&self) {
        unsafe {if self.card_visible {let _=SetWindowPos(self.card,Some(HWND_TOPMOST),0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE);}}
    }
    /// Show, redraw or move the card window. Returns its rectangle in client
    /// coordinates, or None when no card is shown.
    unsafe fn update_card(&mut self,target:&TargetWindow,card:Option<&StepCard>)->Option<[f32;4]> {
        unsafe {
            self.card_content=card.cloned();
            self.card_target=Some(target.clone());
            let Some(card)=card.filter(|_|!self.prefs.hidden) else {self.hide_card();return None;};
            // While the player drags it, the card follows the cursor only.
            if self.drag.is_some() {return self.card_rect;}
            let scale=window_scale(target);
            let client=(target.width as f32,target.height as f32);
            let (default_x,default_y,width)=card_origin(client.0,client.1,scale);
            let state=CardState {card:card.clone(),collapsed:self.prefs.collapsed,
                notice:self.notice.as_ref().map(|n|(n.text.clone(),n.undo.is_some())),scale:scale.to_bits(),client:(target.width,target.height)};
            if self.card_state.as_ref()!=Some(&state) || !self.card_visible {
                let max_height=(client.1*0.8).max(80.0*scale).min(client.1);
                let (open,marks)=(self.open_ref,self.marks.clone());
                let graphics=self.card_graphics.as_mut().unwrap();
                match graphics.draw_card(card,self.prefs.collapsed,self.notice.as_ref(),width,max_height,scale,open,&marks,self.prefs.production_open) {
                    Ok((size,buttons))=>{self.card_size=size;self.card_buttons=buttons;self.card_state=Some(state);}
                    Err(error)=>{
                        self.hide_card();let message=wide(&crate::i18n::format("Rendering suspended: {0}",&[&error.to_string()]));
                        let _=SetWindowTextW(self.status,PCWSTR(message.as_ptr()));return None;
                    }
                }
            }
            let rect=card::place(&self.prefs,(default_x,default_y),self.card_size,client);
            let screen=[target.x+rect[0] as i32,target.y+rect[1] as i32,(rect[2]-rect[0]).ceil() as i32,(rect[3]-rect[1]).ceil() as i32];
            if !self.card_visible || self.card_placed!=Some(screen) {
                if SetWindowPos(self.card,Some(HWND_TOPMOST),screen[0],screen[1],screen[2],screen[3],SWP_NOACTIVATE).is_err() {
                    self.hide_card();return None;
                }
                self.card_placed=Some(screen);
            }
            if !self.card_visible {
                self.card_graphics.as_ref().unwrap().fade_in();
                let _=ShowWindow(self.card,SW_SHOWNOACTIVATE);self.card_visible=true;
            }
            self.card_rect=Some(rect);
            Some(rect)
        }
    }
    /// Draw and show the highlights. The window is moved only when the game's
    /// client area moved and shown only when it was hidden: no hide/show churn.
    /// Returns false if nothing may stay on screen.
    unsafe fn present(&mut self,frame:&OverlayFrame,shown:&OverlayFrame,avoid:Option<[f32;4]>)->windows::core::Result<bool> {
        unsafe {
            let target=&frame.target;
            let placement=[target.x,target.y,target.width as i32,target.height as i32];
            if !self.visible || self.placed!=Some(placement) {
                SetWindowPos(self.overlay,Some(HWND_TOPMOST),target.x,target.y,target.width as i32,target.height as i32,SWP_NOACTIVATE)?;
                self.placed=Some(placement);self.topmost_at=Instant::now();
                self.raise_card();
            }
            let mut marks=shown.clone();marks.card=None;
            let avoid:Vec<[f32;4]>=avoid.into_iter().collect();
            let graphics=self.graphics.as_mut().unwrap();
            graphics.draw(&marks,&avoid)?;
            if !target_matches(target) {return Ok(false);}
            // A highlight that expired while drawing is removed at once.
            if visible_frame(frame,Instant::now()).is_none_or(|again|again.marks.is_empty()) {return Ok(false);}
            if !self.visible {
                graphics.fade_in();
                let _=ShowWindow(self.overlay,SW_SHOWNOACTIVATE);self.visible=true;
            }
            Ok(true)
        }
    }
    unsafe fn hide_overlay(&mut self) {unsafe {if self.visible {let _=ShowWindow(self.overlay,SW_HIDE);} self.visible=false;self.drawn_avoid=None;}}
    unsafe fn hide_card(&mut self) {
        unsafe {
            if self.card_visible {let _=ShowWindow(self.card,SW_HIDE);}
            self.card_visible=false;self.card_rect=None;self.pressed=None;
            if self.drag.take().is_some() {let _=ReleaseCapture();}
        }
    }
    unsafe fn hide(&mut self) {unsafe {self.hide_overlay();self.hide_card();}}
    fn save_prefs(&self) {self.prefs.save(&self.prefs_path);}
    unsafe fn toggle_card(&mut self) {
        unsafe {self.prefs.hidden=!self.prefs.hidden;self.save_prefs();self.card_state=None;self.update(true);}
    }
    /// Cursor position in card coordinates.
    unsafe fn card_point(&self)->Option<(f32,f32)> {
        unsafe {
            let mut point=POINT::default();
            if GetCursorPos(&mut point).is_err() || !ScreenToClient(self.card,&mut point).as_bool() {return None;}
            Some((point.x as f32,point.y as f32))
        }
    }
    unsafe fn card_button_down(&mut self) {
        unsafe {
            let Some((x,y))=self.card_point() else {return;};
            if let Some(button)=card::hit(&self.card_buttons,x,y) {self.pressed=Some(button);}
            else {self.drag=Some(Drag {grab:(x as i32,y as i32),moved:false});}
            SetCapture(self.card);
        }
    }
    unsafe fn card_mouse_move(&mut self) {
        unsafe {
            let (Some(drag),Some(target))=(self.drag.as_mut(),self.card_target.as_ref()) else {return;};
            let mut cursor=POINT::default();if GetCursorPos(&mut cursor).is_err() {return;}
            let (width,height)=self.card_size;
            let client=(target.width as f32,target.height as f32);
            let left=((cursor.x-drag.grab.0-target.x) as f32).clamp(0.0,(client.0-width).max(0.0));
            let top=((cursor.y-drag.grab.1-target.y) as f32).clamp(0.0,(client.1-height).max(0.0));
            let screen=[target.x+left as i32,target.y+top as i32];
            if self.card_placed.is_some_and(|p|p[0]==screen[0]&&p[1]==screen[1]) {return;}
            drag.moved=true;
            let _=SetWindowPos(self.card,None,screen[0],screen[1],0,0,SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE);
            if let Some(placed)=self.card_placed.as_mut() {placed[0]=screen[0];placed[1]=screen[1];}
            self.card_rect=Some([left,top,left+width,top+height]);
        }
    }
    /// End of a drag: remember the place and redraw the highlights around it.
    unsafe fn end_drag(&mut self) {
        unsafe {
            let Some(drag)=self.drag.take() else {return;};
            if drag.moved && let (Some(rect),Some(target))=(self.card_rect,self.card_target.as_ref()) {
                self.prefs.position=card::position_of(rect[0],rect[1],(target.width as f32,target.height as f32));
                self.save_prefs();
            }
            self.update(true);
        }
    }
    unsafe fn card_button_up(&mut self) {
        unsafe {
            let pressed=self.pressed.take();
            let _=ReleaseCapture();
            if self.drag.is_some() {self.end_drag();return;}
            let Some(pressed)=pressed else {return;};
            let Some((x,y))=self.card_point() else {return;};
            if card::hit(&self.card_buttons,x,y)!=Some(pressed) {return;}
            match pressed {
                Button::Hide=>{self.prefs.hidden=true;self.save_prefs();self.hide_card();self.update(true);}
                Button::Collapse=>{self.prefs.collapsed=!self.prefs.collapsed;self.save_prefs();self.card_state=None;self.update(true);}
                Button::Production=>{self.prefs.production_open=!self.prefs.production_open;self.save_prefs();self.card_state=None;self.update(true);}
                Button::Tag(i)=>{
                    let i=i as usize;self.open_ref=if self.open_ref==Some(i) {None} else {Some(i)};
                    self.card_state=None;self.update(true);
                }
                Button::Item(j)=>{
                    if let (Some(open),Some(card))=(self.open_ref,self.card_content.as_ref())
                        && let Some(row)=card.references.get(open).and_then(|s|s.rows.get(j as usize)) {
                        self.marks.toggle(&card.references_scope,&row.id);self.marks.save(&self.marks_path);
                    }
                    self.card_state=None;self.update(true);
                }
                Button::Done=>{
                    if let Some(card)=self.card_content.as_ref().filter(|c|c.checkable&&!c.key.is_empty())
                        && let Ok(mut s)=self.shared.lock() && s.actions.len()<32 {
                        s.actions.push_back(ControlAction::Check {guide_id:card.guide_id.clone(),key:card.key.clone(),checked:true});
                    }
                    self.wake.notify();
                }
                Button::SetAside|Button::Resume(_)=>{
                    let target=self.card_content.as_ref().filter(|c|!c.guide_id.is_empty()).and_then(|c|match pressed {
                        Button::Resume(i)=>c.set_aside.get(i as usize).map(|(key,_)|(c.guide_id.clone(),key.clone(),false)),
                        _=>(c.can_set_aside&&!c.key.is_empty()).then(||(c.guide_id.clone(),c.key.clone(),true)),
                    });
                    if let Some((guide_id,key,aside))=target && let Ok(mut s)=self.shared.lock() && s.actions.len()<32 {
                        s.actions.push_back(ControlAction::SetAside {guide_id,key,aside});
                    }
                    self.wake.notify();
                }
                Button::ConfirmAnswer=>{
                    if let Some(card)=self.card_content.as_ref().filter(|c|!c.guide_id.is_empty())
                        && let Some(answer)=card.answer.as_ref()
                        && let Ok(mut s)=self.shared.lock() && s.actions.len()<32 {
                        s.actions.push_back(ControlAction::ConfirmAnswer {guide_id:card.guide_id.clone(),key:answer.key.clone(),
                            quote:answer.quote.clone(),event_id:answer.event_id.clone(),option_id:answer.option_id.clone()});
                    }
                    self.wake.notify();
                }
                Button::Undo=>{
                    if let Some(undo)=self.notice.as_ref().and_then(|n|n.undo.as_ref())
                        && let Ok(mut s)=self.shared.lock() && s.actions.len()<32 {
                        s.actions.push_back(undo_action(undo));
                    }
                    self.wake.notify();
                }
            }
        }
    }
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
                CARD_TOGGLE=>self.toggle_card(),
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
                WM_HOTKEY if wparam.0==CARD_HOTKEY as usize=>{ui.toggle_card();return LRESULT(0);}
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
                WM_MOUSEACTIVATE if window==ui.overlay||window==ui.card=>return LRESULT(MA_NOACTIVATE as isize),
                WM_ERASEBKGND if window==ui.overlay||window==ui.card=>return LRESULT(1),
                WM_SETCURSOR if window==ui.card=>{
                    let over=ui.card_point().and_then(|(x,y)|card::hit(&ui.card_buttons,x,y));
                    let _=SetCursor(LoadCursorW(None,if over.is_some() {IDC_HAND}else{IDC_SIZEALL}).ok());
                    return LRESULT(1);
                }
                WM_LBUTTONDOWN if window==ui.card=>{ui.card_button_down();return LRESULT(0);}
                WM_MOUSEMOVE if window==ui.card=>{ui.card_mouse_move();return LRESULT(0);}
                WM_LBUTTONUP if window==ui.card=>{ui.card_button_up();return LRESULT(0);}
                WM_CAPTURECHANGED if window==ui.card=>{ui.pressed=None;ui.end_drag();return LRESULT(0);}
                _=>{}
            }
        }
        DefWindowProcW(window,message,wparam,lparam)
    }
}

unsafe fn run(shared:Arc<Mutex<Shared>>,paused:Arc<AtomicBool>,exit:Arc<AtomicBool>,
    wake:Arc<Wake>,discreet:bool,managed:bool,prefs_path:std::path::PathBuf,
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
        let card_window=CreateWindowExW(WS_EX_TOPMOST|WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW|WS_EX_NOREDIRECTIONBITMAP,
            class.lpszClassName,PCWSTR(wide(&crate::i18n::text("HOI4 Companion · Step card")).as_ptr()),WS_POPUP,0,0,1,1,None,None,Some(instance),None).map_err(|e|e.to_string())?;
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
        child(w!("BUTTON"),PCWSTR(wide(&crate::i18n::text("Show/hide step card (Ctrl+Alt+H)")).as_ptr()),WS_TABSTOP,296,126,206,32,CARD_TOGGLE).map_err(|e|e.to_string())?;
        SetWindowDisplayAffinity(overlay,WDA_EXCLUDEFROMCAPTURE).map_err(|e|e.to_string())?;
        SetWindowDisplayAffinity(card_window,WDA_EXCLUDEFROMCAPTURE).map_err(|e|e.to_string())?;
        let graphics=Graphics::new(overlay).map_err(|e|e.to_string())?;
        let card_graphics=Graphics::new(card_window).map_err(|e|e.to_string())?;
        let prefs=Prefs::load(&prefs_path);
        let marks_path=crate::card::Marks::path(&prefs_path);
        let marks=crate::card::Marks::load(&marks_path);
        let mut ui=Box::new(Ui {overlay,card:card_window,status,pause,combo,shared,paused,exit,
            graphics:Some(graphics),card_graphics:Some(card_graphics),last_revision:u64::MAX,last_guides:Vec::new(),last_selected:String::new(),
            last_status:String::new(),excluded:true,visible:false,drawn_avoid:None,placed:None,topmost_at:Instant::now(),
            card_visible:false,card_placed:None,card_state:None,card_size:(0.0,0.0),card_buttons:Vec::new(),card_rect:None,
            card_target:None,card_content:None,notice:None,prefs,prefs_path,open_ref:None,marks,marks_path,drag:None,pressed:None,
            _font:font,last_paused:false,wake:wake.clone()});
        let pointer=(&mut *ui) as *mut Ui as isize;
        SetWindowLongPtrW(overlay,GWLP_USERDATA,pointer);SetWindowLongPtrW(control,GWLP_USERDATA,pointer);SetWindowLongPtrW(card_window,GWLP_USERDATA,pointer);
        SetTimer(Some(control),1,50,None);
        // A global shortcut owned by the HUD; it sends nothing to the game.
        let hotkey=RegisterHotKey(Some(control),CARD_HOTKEY,MOD_CONTROL|MOD_ALT|MOD_NOREPEAT,0x48).is_ok();
        FOREGROUND_WAKE.with(|slot|*slot.borrow_mut()=Some(wake));
        let foreground=ForegroundEvents(SetWinEventHook(EVENT_SYSTEM_FOREGROUND,EVENT_SYSTEM_FOREGROUND,None,
            Some(foreground_changed),0,0,WINEVENT_OUTOFCONTEXT));
        if let Ok(mut state)=ui.shared.lock(){state.foreground_events = !foreground.0.0.is_null();}
        let mut tray=NOTIFYICONDATAW {cbSize:std::mem::size_of::<NOTIFYICONDATAW>() as u32,hWnd:control,uID:1,
            uFlags:NIF_MESSAGE|NIF_ICON|NIF_TIP,uCallbackMessage:TRAY,..Default::default()};
        let tip=wide("HOI4 Companion");tray.szTip[..tip.len()].copy_from_slice(&tip);
        tray.hIcon=LoadIconW(None,IDI_APPLICATION).unwrap_or_default();
        let tray_added=discreet && !managed && Shell_NotifyIconW(NIM_ADD,&tray).as_bool();
        if let Ok(mut s)=ui.shared.lock(){s.tray_controls=tray_added;}
        // Keep the legacy panel visible if notification-area registration fails.
        if !tray_added && !managed {let _=ShowWindow(control,SW_SHOWNOACTIVATE);}
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
        if hotkey {let _=UnregisterHotKey(Some(control),CARD_HOTKEY);}
        ui.hide();KillTimer(Some(control),1).ok();
        SetWindowLongPtrW(overlay,GWLP_USERDATA,0);SetWindowLongPtrW(control,GWLP_USERDATA,0);SetWindowLongPtrW(card_window,GWLP_USERDATA,0);
        let _=DestroyWindow(overlay);let _=DestroyWindow(card_window);let _=DestroyWindow(control);
        ui.graphics=None;ui.card_graphics=None;
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
        marks:vec![Highlight {rect:[10.0,20.0,80.0,60.0],caption:"Ricerca".into(),role:Role::Next,panel:"research".into()}],
        card:None,observed_at:Instant::now(),valid_for:Duration::from_secs(1),card_observed_at:Instant::now()}}
    #[test]
    fn expired_highlights_leave_the_card_and_each_layer_keeps_its_own_deadline() {
        let now=Instant::now();
        let mut f=frame();f.valid_for=Duration::from_millis(750);
        f.observed_at=now;f.card_observed_at=now;f.card=Some(StepCard {heading:"Build".into(),..Default::default()});
        let shown=visible_frame(&f,now+Duration::from_millis(700)).unwrap();
        assert_eq!((shown.marks.len(),shown.card.is_some()),(1,true));
        let shown=visible_frame(&f,now+Duration::from_millis(751)).unwrap();
        assert!(shown.marks.is_empty(),"the 750 ms highlight deadline is unchanged");
        assert!(shown.card.is_some(),"the card does not disappear with the highlights");
        assert!(visible_frame(&f,now+CARD_VALID_FOR+Duration::from_millis(1)).is_none());
        let mut fresh_card=f.clone();fresh_card.card_observed_at=now+Duration::from_secs(1);
        let shown=visible_frame(&fresh_card,now+Duration::from_secs(1)).unwrap();
        assert!(shown.marks.is_empty()&&shown.card.is_some(),"a refreshed card never revives expired highlights");
        let mut bare=f;bare.card=None;
        assert!(visible_frame(&bare,now+Duration::from_millis(751)).is_none());
    }
    #[test]
    fn captions_keep_their_side_while_it_stays_free() {
        let mut f=frame();f.marks[0].rect=[300.0,200.0,370.0,240.0];
        let first=caption_slots(&f,1.0,&[],&HashMap::new())[0].unwrap();
        assert_eq!(first.0,0,"first fit is above the control");
        // A previous frame placed this caption on the right: keep it there
        // although the first-fit side is free again.
        let preferred=HashMap::from([(caption_key(&f.marks[0]),1usize)]);
        let kept=caption_slots(&f,1.0,&[],&preferred)[0].unwrap();
        assert_eq!(kept.0,1);assert!(kept.1[0]>=370.0);
        // A small pan moves the caption with its control on the same side.
        f.marks[0].rect=[303.0,201.0,373.0,241.0];
        let moved=caption_slots(&f,1.0,&[],&preferred)[0].unwrap();
        assert_eq!(moved.0,1);assert!((moved.1[0]-kept.1[0]-3.0).abs()<0.01);
        // When the kept side becomes blocked, fall back to the first free one.
        let blocked=caption_slots(&f,1.0,&[[380.0,190.0,700.0,260.0]],&preferred)[0].unwrap();
        assert_ne!(blocked.0,1);
        assert!(!overlaps(blocked.1,[380.0,190.0,700.0,260.0],0.0));
    }
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
                caption:"Tank · requested factories and configuration".into(),role:Role::Next,panel:"production".into()}).collect();
            // The add-category cue can be close to the first row without its
            // caption covering any game controls or colliding with the row cue.
            f.marks.push(Highlight {rect:[100.0*scale,90.0*scale,170.0*scale,120.0*scale],caption:"Add tank".into(),role:Role::Next,panel:"production".into()});
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
    fn step_card_sits_right_under_the_top_bar_and_captions_avoid_it() {
        for (w,h,scale) in [(1920.0,1080.0,1.0),(2560.0,1440.0,1.5),(1280.0,720.0,1.0)] {
            let (left,top,width)=card_origin(w,h,scale);
            assert!(left>w/2.0,"right half");assert!((left+width-(w-16.0*scale)).abs()<0.01);
            assert!(top>=96.0*scale&&top<h/2.0,"below the top bar and theater selector");
        }
        let mut f=frame();f.marks[0].rect=[560.0,100.0,700.0,140.0];
        let avoid=[[420.0,40.0,790.0,400.0]];
        let placed=caption_layout_avoiding(&f,1.0,&avoid)[0];
        assert!(placed.is_none_or(|r|!overlaps(r,avoid[0],0.0)));
        let mut card=frame();card.marks.clear();card.card=Some(StepCard {heading:"Build".into(),..Default::default()});
        assert!(validate_frame(&card).is_ok());
        card.card.as_mut().unwrap().body="x".repeat(2001);assert!(validate_frame(&card).is_err());
    }
    #[test]
    fn completed_production_keeps_border_without_caption_and_narrow_view_never_covers_controls() {
        let mut f=frame();f.marks[0].panel="production".into();f.marks[0].role=Role::Active;
        assert_eq!(caption_layout(&f,1.0),vec![None]);
        f.marks[0].role=Role::Next;f.marks[0].rect=[0.0,60.0,800.0,150.0];
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
