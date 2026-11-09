//! Keeps one native change-event subscription while the HUD is alive. Payloads
//! are not reinterpreted here; coherent projections remain backend-owned.
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
pub struct Subscription {
    closed: Arc<AtomicBool>,
    socket: Arc<Mutex<Option<TcpStream>>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Subscription {
    pub fn start(port: u16) -> Result<Self, String> {
        let closed = Arc::new(AtomicBool::new(false));
        let socket = Arc::new(Mutex::new(None));
        let stop = closed.clone();
        let connection = socket.clone();
        let worker=thread::Builder::new().name("native-guide-events".into()).spawn(move||{
            while !stop.load(Ordering::Acquire){
                let stream=TcpStream::connect_timeout(&([127,0,0,1],port).into(),Duration::from_secs(1));
                if let Ok(mut stream)=stream{
                    let _=stream.set_read_timeout(Some(Duration::from_secs(1)));let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
                    if let Ok(clone)=stream.try_clone() && let Ok(mut slot)=connection.lock(){*slot=Some(clone);}
                    let request=format!("GET /api/state/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
                    if stream.write_all(request.as_bytes()).is_ok(){let mut bytes=[0u8;4096];while !stop.load(Ordering::Acquire){match stream.read(&mut bytes){Ok(0)=>break,Ok(_)=>{},Err(e) if [std::io::ErrorKind::TimedOut,std::io::ErrorKind::WouldBlock].contains(&e.kind())=>continue,Err(_)=>break}}}
                    if let Ok(mut slot)=connection.lock(){*slot=None;}
                }
                for _ in 0..5{if stop.load(Ordering::Acquire){break;}thread::sleep(Duration::from_millis(100));}
            }
        }).map_err(|e|e.to_string())?;
        Ok(Self {
            closed,
            socket,
            worker: Some(worker),
        })
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(slot) = self.socket.lock()
            && let Some(socket) = slot.as_ref()
        {
            let _ = socket.shutdown(Shutdown::Both);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
