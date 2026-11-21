//! Wake idle recognition on external window/UI events without losing a signal
//! between foreground inspection and the subsequent bounded wait.
use std::sync::{Condvar,Mutex};
use std::time::Duration;
use windows::Win32::Media::{timeBeginPeriod,timeEndPeriod,TIMERR_NOERROR};
use windows::Win32::System::Threading::{GetCurrentProcess,ProcessPowerThrottling,SetProcessInformation,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION,PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,PROCESS_POWER_THROTTLING_STATE};

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

/// The HUD loop waits on a condition variable (`Wake::wait`) and a channel
/// (`Reads::wait`). On Windows both time out on the system timer tick
/// (~15.6 ms by default), so a 16 ms frame became 31 ms (HUD plan part 2,
/// step 1). Held for the HUD process lifetime, this asks for a 1 ms tick for
/// this process only (Windows 10 2004 and later), and asks Windows 11 to keep
/// it while the overlay window is hidden or covered.
pub struct FineTimer { active:bool }
impl FineTimer {
    pub fn begin()->Self {
        let keep=PROCESS_POWER_THROTTLING_STATE {Version:PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask:PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,StateMask:0};
        // Best effort: without it the waits are only coarser, never wrong.
        unsafe {
            let _=SetProcessInformation(GetCurrentProcess(),ProcessPowerThrottling,(&raw const keep).cast(),size_of_val(&keep) as u32);
            Self {active:timeBeginPeriod(1)==TIMERR_NOERROR}
        }
    }
}
impl FineTimer {
    /// Windows granted the 1 ms tick (written in the trace start line).
    pub fn active(&self)->bool {self.active}
}
impl Drop for FineTimer {
    fn drop(&mut self) {if self.active {unsafe {let _=timeEndPeriod(1);}}}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc,mpsc};
    use std::time::Instant;
    /// Median time of a 5 ms wait on each primitive the HUD loop waits on.
    fn waits_ms()->(f64,f64) {
        let wake=Wake::default();let (_keep,receive)=mpsc::channel::<()>();
        let median=|mut v:Vec<f64>|{v.sort_by(f64::total_cmp);v[v.len()/2]};
        let timed=|f:&dyn Fn()|{let began=Instant::now();f();began.elapsed().as_secs_f64()*1000.};
        let condvar=median((0..21).map(|_|timed(&||wake.wait(wake.revision(),Duration::from_millis(5)))).collect());
        let channel=median((0..21).map(|_|timed(&||{let _=receive.recv_timeout(Duration::from_millis(5));})).collect());
        (condvar,channel)
    }
    #[test]
    fn a_fine_timer_keeps_short_waits_near_their_timeout() {
        let coarse=waits_ms();
        let fine={let _timer=FineTimer::begin();waits_ms()};
        eprintln!("5 ms waits, median ms (condvar, channel): default tick {coarse:?}, fine timer {fine:?}");
        assert!(fine.0<9.&&fine.1<9.,"a 5 ms wait took {fine:?} ms with a 1 ms tick");
    }
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
