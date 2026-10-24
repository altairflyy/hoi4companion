//! WGC capture of one caller-verified HWND. No hooks, input or process memory.
//! The event callback retains at most one frame; CPU readback happens on demand.
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::core::{factory, Error, Interface, Result, HRESULT};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::{DirectXPixelFormat, Direct3D11::IDirect3DDevice};
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG, HMODULE, HWND, POINT, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

pub(crate) fn error(message: impl AsRef<str>) -> Error { Error::new(E_FAIL, message.as_ref()) }

/// Must stay on its creating thread, and balances our own COM initialization.
pub(crate) struct Apartment { owned: bool, _thread: PhantomData<Rc<()>> }
impl Apartment {
    pub(crate) fn new() -> Result<Self> {
        let owned = match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => true,
            Err(e) if e.code() == HRESULT(0x80010106u32 as i32) => false,
            Err(e) => return Err(e),
        };
        Ok(Self { owned, _thread: PhantomData })
    }
}
impl Drop for Apartment {
    fn drop(&mut self) { if self.owned { unsafe { RoUninitialize() } } }
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed BGRA8, cropped to the actual HWND client rectangle.
    pub bgra: Vec<u8>,
    pub client_x: i32,
    pub client_y: i32,
    pub generation: u64,
    /// Time at which Windows delivered this frame, not time after OCR.
    pub captured_at: Instant,
}

struct Pending { frame: Direct3D11CaptureFrame, generation: u64, at: Instant }
#[derive(Default)]
struct Latest { pending: Option<Pending>, generation: u64, closed: bool, failure: Option<String> }
struct Signal { latest: Mutex<Latest>, changed: Condvar }

pub struct Capture {
    hwnd: HWND,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt_device: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    item: GraphicsCaptureItem,
    frame_token: i64,
    close_token: i64,
    signal: Arc<Signal>,
    size: SizeInt32,
    staging: Option<ID3D11Texture2D>,
    _apartment: Apartment,
}

impl Capture {
    /// Caller must independently verify that `hwnd` belongs to HOI4.
    pub fn new(hwnd: isize) -> Result<Self> {
        if hwnd == 0 { return Err(Error::new(E_INVALIDARG,"Null capture window")); }
        let apartment=Apartment::new()?;
        if !GraphicsCaptureSession::IsSupported()? {return Err(error("Windows Graphics Capture unavailable"));}
        let hwnd=HWND(hwnd as *mut _);
        let mut device=None;let mut context=None;
        unsafe { D3D11CreateDevice(None,D3D_DRIVER_TYPE_HARDWARE,HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,None,D3D11_SDK_VERSION,Some(&mut device),None,Some(&mut context))?; }
        let device=device.ok_or_else(||error("D3D11 device not returned"))?;
        let context=context.ok_or_else(||error("D3D11 context not returned"))?;
        let dxgi:IDXGIDevice=device.cast()?;
        let winrt_device:IDirect3DDevice=unsafe {CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?}.cast()?;
        let interop:IGraphicsCaptureItemInterop=factory::<GraphicsCaptureItem,IGraphicsCaptureItemInterop>()?;
        let item:GraphicsCaptureItem=unsafe {interop.CreateForWindow(hwnd)?};
        let size=item.Size()?;
        if size.Width<=0 || size.Height<=0 {return Err(error("Capture window has no visible size"));}
        let pool=Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt_device,DirectXPixelFormat::B8G8R8A8UIntNormalized,2,size)?;
        let signal=Arc::new(Signal {latest:Mutex::new(Latest::default()),changed:Condvar::new()});
        let callback_signal=signal.clone();
        let frame_token=pool.FrameArrived(&TypedEventHandler::new(move |sender:windows::core::Ref<'_,Direct3D11CaptureFramePool>,_|{
            if let Some(sender)=sender.as_ref() {
                match sender.TryGetNextFrame() {
                    Ok(frame)=>{
                        let mut state=callback_signal.latest.lock().unwrap_or_else(|e|e.into_inner());
                        if state.closed {let _=frame.Close();return Ok(());}
                        state.generation=state.generation.wrapping_add(1);
                        let generation=state.generation;
                        if let Some(previous)=state.pending.replace(Pending {frame,generation,at:Instant::now()}) {let _=previous.frame.Close();}
                        callback_signal.changed.notify_one();
                    },
                    Err(e)=>{
                        let mut state=callback_signal.latest.lock().unwrap_or_else(|e|e.into_inner());
                        state.failure=Some(e.to_string());callback_signal.changed.notify_one();
                    },
                }
            }
            Ok(())
        }))?;
        let closed_signal=signal.clone();
        let close_token=item.Closed(&TypedEventHandler::new(move |_,_|{
            closed_signal.latest.lock().unwrap_or_else(|e|e.into_inner()).closed=true;
            closed_signal.changed.notify_all();Ok(())
        }))?;
        let session=pool.CreateCaptureSession(&item)?;
        // Cursor pixels are not evidence about a GUI control.
        session.SetIsCursorCaptureEnabled(false)?;
        session.StartCapture()?;
        Ok(Self {hwnd,device,context,winrt_device,pool,session,item,frame_token,close_token,
            signal,size,staging:None,_apartment:apartment})
    }

    pub fn wait_latest(&mut self, timeout:Duration) -> Result<Option<Frame>> {
        let signal=self.signal.clone();
        let state=signal.latest.lock().unwrap_or_else(|e|e.into_inner());
        let (mut state,_)=signal.changed.wait_timeout_while(state,timeout,|s|s.pending.is_none()&&!s.closed&&s.failure.is_none())
            .unwrap_or_else(|e|e.into_inner());
        if state.closed {return Err(error("Capture window closed"));}
        if let Some(failure)=state.failure.take() {return Err(error(failure));}
        let Some(pending)=state.pending.take() else {return Ok(None);};
        drop(state);
        let content=pending.frame.ContentSize()?;
        if content.Width!=self.size.Width || content.Height!=self.size.Height {
            pending.frame.Close()?;
            if let Some(other)=signal.latest.lock().unwrap_or_else(|e|e.into_inner()).pending.take() {let _=other.frame.Close();}
            if content.Width<=0||content.Height<=0 {return Ok(None);}
            self.pool.Recreate(&self.winrt_device,DirectXPixelFormat::B8G8R8A8UIntNormalized,2,content)?;
            self.size=content;self.staging=None;
            return Ok(None);
        }
        let result=self.read_frame(&pending);
        let _=pending.frame.Close();
        result.map(Some)
    }

    fn read_frame(&mut self,pending:&Pending)->Result<Frame> {
        let access:IDirect3DDxgiInterfaceAccess=pending.frame.Surface()?.cast()?;
        let texture:ID3D11Texture2D=unsafe {access.GetInterface()?};
        let mut description=D3D11_TEXTURE2D_DESC::default();unsafe {texture.GetDesc(&mut description)};
        if description.Width<self.size.Width as u32 || description.Height<self.size.Height as u32 {
            return Err(error("Capture surface is smaller than qualified content"));
        }
        if self.staging.is_none() {
            let description=D3D11_TEXTURE2D_DESC {Usage:D3D11_USAGE_STAGING,BindFlags:0,
                CPUAccessFlags:D3D11_CPU_ACCESS_READ.0 as u32,MiscFlags:0,..description};
            let mut staging=None;unsafe {self.device.CreateTexture2D(&description,None,Some(&mut staging))?};
            self.staging=staging;
        }
        let staging=self.staging.as_ref().ok_or_else(||error("No staging texture"))?;
        let mut bounds=RECT::default();let mut client=RECT::default();let mut origin=POINT::default();
        unsafe {
            DwmGetWindowAttribute(self.hwnd,DWMWA_EXTENDED_FRAME_BOUNDS,(&mut bounds as *mut RECT).cast(),size_of::<RECT>() as u32)?;
            GetClientRect(self.hwnd,&mut client)?;
            ClientToScreen(self.hwnd,&mut origin).ok()?;
        }
        if bounds.right-bounds.left!=self.size.Width || bounds.bottom-bounds.top!=self.size.Height {
            return Err(error("Capture/client geometry changed; wait for a fresh frame"));
        }
        let x=origin.x-bounds.left;let y=origin.y-bounds.top;
        let width=client.right-client.left;let height=client.bottom-client.top;
        if x<0||y<0||width<=0||height<=0||x+width>self.size.Width||y+height>self.size.Height {
            return Err(error("Unqualified capture/client bounds"));
        }
        let bytes=(width as usize).checked_mul(height as usize).and_then(|n|n.checked_mul(4))
            .filter(|n|*n<=128_000_000).ok_or_else(||error("Capture exceeds image budget"))?;
        unsafe {self.context.CopyResource(staging,&texture)};
        let mut mapped=D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {self.context.Map(staging,0,D3D11_MAP_READ,0,Some(&mut mapped))?};
        let mut bgra=vec![0;bytes];
        if mapped.pData.is_null() || mapped.RowPitch<(x+width) as u32*4 {
            unsafe {self.context.Unmap(staging,0)};
            return Err(error("Invalid GPU readback row"));
        }
        for row in 0..height as usize {
            let source=unsafe {(mapped.pData as *const u8).add((y as usize+row)*mapped.RowPitch as usize+x as usize*4)};
            unsafe {std::ptr::copy_nonoverlapping(source,bgra.as_mut_ptr().add(row*width as usize*4),width as usize*4)};
        }
        unsafe {self.context.Unmap(staging,0)};
        Ok(Frame {width:width as u32,height:height as u32,bgra,client_x:origin.x,client_y:origin.y,
            generation:pending.generation,captured_at:pending.at})
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _=self.pool.RemoveFrameArrived(self.frame_token);
        let _=self.item.RemoveClosed(self.close_token);
        let mut state=self.signal.latest.lock().unwrap_or_else(|e|e.into_inner());
        state.closed=true;
        if let Some(pending)=state.pending.take() {let _=pending.frame.Close();}
        drop(state);
        let _=self.session.Close();let _=self.pool.Close();
    }
}
