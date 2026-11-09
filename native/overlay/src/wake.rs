//! Wake idle recognition on external window/UI events without losing a signal
//! between foreground inspection and the subsequent bounded wait.
use std::sync::{Condvar,Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct Wake { generation:Mutex<u64>, changed:Condvar }
impl Wake {
    pub fn revision(&self)->u64 {*self.generation.lock().unwrap_or_else(|e|e.into_inner())}
    pub fn notify(&self) {
        let mut value=self.generation.lock().unwrap_or_else(|e|e.into_inner());
        *value=value.wrapping_add(1);self.changed.notify_all();
    }
    pub fn wait(&self,previous:u64,timeout:Duration) {
        let value=self.generation.lock().unwrap_or_else(|e|e.into_inner());
        let _result=self.changed.wait_timeout_while(value,timeout,|value|*value==previous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc,mpsc};
    #[test]
    fn event_before_wait_is_not_lost_and_duplicate_events_are_safe() {
        let wake=Wake::default();let revision=wake.revision();wake.notify();wake.notify();
        let began=std::time::Instant::now();wake.wait(revision,Duration::from_secs(2));
        assert!(began.elapsed()<Duration::from_secs(1));
        assert_eq!(wake.revision(),revision+2);
    }
    #[test]
    fn event_during_wait_wakes_the_recognizer() {
        let wake=Arc::new(Wake::default());let revision=wake.revision();
        let (send,receive)=mpsc::channel();let (done,completed)=mpsc::channel();let worker=wake.clone();
        let thread=std::thread::spawn(move||{send.send(()).unwrap();worker.wait(revision,Duration::from_secs(2));done.send(()).unwrap();});
        receive.recv().unwrap();wake.notify();completed.recv_timeout(Duration::from_secs(1)).unwrap();thread.join().unwrap();
        assert_ne!(wake.revision(),revision);
    }
}
