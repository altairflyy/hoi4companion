//! Bounded fallback work. Slow OCR/catalog matching never owns the native
//! observation loop. One job only; hidden/native transitions invalidate results.
use std::{sync::mpsc::{self,Receiver,SyncSender},thread::{self,JoinHandle}};
use serde_json::{Value,json};
use crate::{capture::Frame,ocr::{Ocr,OcrPage},protocol::Bridge};

struct Background<J:Send+'static,R:Send+'static> {
    input:Option<SyncSender<(u64,J)>>,
    output:Receiver<(u64,R)>,
    thread:Option<JoinHandle<()>>,
    busy:bool,
    epoch:u64,
}
impl<J:Send+'static,R:Send+'static> Background<J,R> {
    fn new<F:FnMut(J)->R+'static>(factory:impl FnOnce()->F+Send+'static)->Result<Self,String> {
        let (input,receive)=mpsc::sync_channel::<(u64,J)>(1);
        let (send,output)=mpsc::sync_channel(1);
        let thread=thread::Builder::new().name("hud-visual-fallback".into()).spawn(move||{
            let mut process=factory();
            while let Ok((epoch,job))=receive.recv() {
                let result=process(job);
                if send.send((epoch,result)).is_err(){break;}
            }
        }).map_err(|e|e.to_string())?;
        Ok(Self{input:Some(input),output,thread:Some(thread),busy:false,epoch:0})
    }
    fn submit(&mut self,job:J)->Result<(),String> {
        if self.busy{return Err("Visual fallback already has a pending job".into());}
        self.input.as_ref().ok_or("Visual fallback stopped")?.try_send((self.epoch,job)).map_err(|e|e.to_string())?;
        self.busy=true;Ok(())
    }
    fn take(&mut self)->Option<R> {
        match self.output.try_recv() {
            Ok((epoch,result))=>{self.busy=false;(epoch==self.epoch).then_some(result)},
            Err(mpsc::TryRecvError::Empty)=>None,
            Err(mpsc::TryRecvError::Disconnected)=>{self.busy=false;None},
        }
    }
    fn invalidate(&mut self){self.epoch=self.epoch.wrapping_add(1);let _=self.take();}
}
impl<J:Send+'static,R:Send+'static> Drop for Background<J,R> {
    fn drop(&mut self){self.input.take();if let Some(thread)=self.thread.take(){let _=thread.join();}}
}
pub(crate) struct Job {
    pub frame:Frame,
    pub digest:u64,
    pub context:Value,
    pub page:Option<OcrPage>,
}
pub(crate) struct Completed {
    pub job:Job,
    pub page:OcrPage,
    pub response:Value,
    pub ocr_ms:f64,
    pub did_ocr:bool,
}
pub(crate) struct VisualWorker {worker:Background<Job,Result<Completed,String>>}
impl VisualWorker {
    pub fn new(port:u16)->Result<Self,String> {
        // COM/WinRT and the fallback transport are created and dropped inside
        // their own thread, never moved across apartments.
        let worker=Background::new(move ||{
          let mut engine:Option<Ocr>=None;
          let mut bridge:Option<Bridge>=None;
          move |job:Job|{
            let did_ocr=job.page.is_none();
            let began=std::time::Instant::now();
            let page=if let Some(page)=&job.page{page.clone()}else{
                if engine.is_none(){engine=Some(Ocr::new().map_err(|e|e.to_string())?);}
                engine.as_ref().unwrap().recognize(&job.frame,2).map_err(|e|e.to_string())?
            };
            let ocr_ms=began.elapsed().as_secs_f64()*1000.;
            if bridge.is_none(){bridge=Some(Bridge::new(port)?);}
            let response=bridge.as_mut().unwrap().request(json!({"kind":"frame","version":job.context["version"],"view":job.context["view"],
                "width":job.frame.width,"height":job.frame.height,"raw_len":job.frame.bgra.len(),"ocr":page,
                "frame_hash":format!("{:x}",job.digest)}),&job.frame.bgra)?;
            Ok(Completed{job,page,response,ocr_ms,did_ocr})
          }
        })?;
        Ok(Self{worker})
    }
    pub fn busy(&self)->bool{self.worker.busy}
    pub fn submit(&mut self,job:Job)->Result<(),String>{self.worker.submit(job)}
    pub fn take(&mut self)->Option<Result<Completed,String>>{self.worker.take()}
    pub fn invalidate(&mut self){self.worker.invalidate();}
}
pub(crate) fn context(snapshot:&Value,view:Value)->Value {
    json!({"guide_id":snapshot["guide_id"],"server_id":snapshot["guide_server_id"],"version":snapshot["version"],"view":view})
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn slow_fallback_does_not_block_native_observation_and_cannot_queue_frames(){
        let (started,ready)=mpsc::channel();let (release,wait)=mpsc::channel();
        let mut worker=Background::new(move||move|job:u32|{started.send(()).unwrap();wait.recv().unwrap();job}).unwrap();
        worker.submit(1).unwrap();ready.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
        assert!(worker.take().is_none());
        assert!(worker.submit(2).is_err());
        worker.invalidate(); // Opening a native tree never waits for OCR.
        release.send(()).unwrap();
        let end=std::time::Instant::now()+std::time::Duration::from_secs(1);
        while worker.busy&&std::time::Instant::now()<end{assert!(worker.take().is_none());thread::yield_now();}
        assert!(!worker.busy,"Old map result must be discarded after native activation");
    }
    #[test]fn visual_result_context_changes_with_guide_server_version_and_view(){
        let source=json!({"guide_id":"g","guide_server_id":"s","version":1});
        let view=json!([1,2,0,0,100,100]);let original=context(&source,view.clone());
        for (key,value) in [("guide_id",json!("other")),("guide_server_id",json!("other")),("version",json!(2))]{
            let mut changed=source.clone();changed[key]=value;assert_ne!(context(&changed,view.clone()),original);
        }
        assert_ne!(context(&source,json!([3,2,0,0,100,100])),original);
    }
}
