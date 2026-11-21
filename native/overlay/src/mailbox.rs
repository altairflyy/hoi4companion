//! Backend requests off the HUD loop (HUD plan step 4). The loop never waits
//! on HTTP: drawing, card drag and buttons stay live while the backend is slow.
//!
//! `Reads` has one request in flight at a time; the loop takes the finished
//! one when it is there and the newest finished one wins. After a control
//! action changes the backend state, `discard_sent` drops every read already
//! sent, so a reply from before the change is never shown after it.
//!
//! `Calls` is a FIFO of control actions (Done, Undo, select…) on a thread of
//! their own, each with its own reply, so a slow action never holds back the
//! reads and a slow read never holds back an action.
//!
//! Each worker builds its state inside its own thread (the transport is never
//! moved across threads); a failure to build it is returned by the constructor.
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

/// A finished request: what was asked, when the worker sent it and got the
/// answer, and the answer.
pub struct Done<Q, R> {
    pub request: Q,
    pub sent: Instant,
    pub received: Instant,
    pub value: R,
}

type Channels<Q, R> = (Sender<(u64, Q)>, Receiver<(u64, Done<Q, R>)>);

fn spawn<Q, R, F>(name: &str, factory: impl FnOnce() -> Result<F, String> + Send + 'static) -> Result<Channels<Q, R>, String>
where
    Q: Send + 'static,
    R: Send + 'static,
    F: FnMut(&Q) -> R,
{
    let (input, requests) = mpsc::channel::<(u64, Q)>();
    let (answers, output) = mpsc::channel();
    let (ready, started) = mpsc::sync_channel(1);
    // Not joined on drop: a request in flight ends on its own timeout and the
    // thread then sees the closed queue.
    thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let mut process = match factory() {
                Ok(process) => {
                    let _ = ready.send(Ok(()));
                    process
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            while let Ok((sequence, request)) = requests.recv() {
                let sent = Instant::now();
                let value = process(&request);
                let done = Done { request, sent, received: Instant::now(), value };
                if answers.send((sequence, done)).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    started.recv().map_err(|_| format!("{name} did not start"))??;
    Ok((input, output))
}

pub struct Reads<Q, R> {
    input: Sender<(u64, Q)>,
    output: Receiver<(u64, Done<Q, R>)>,
    busy: bool,
    next: u64,
    accept_from: u64,
}
impl<Q: Send + 'static, R: Send + 'static> Reads<Q, R> {
    pub fn new<F: FnMut(&Q) -> R>(name: &str, factory: impl FnOnce() -> Result<F, String> + Send + 'static) -> Result<Self, String> {
        let (input, output) = spawn(name, factory)?;
        Ok(Self { input, output, busy: false, next: 0, accept_from: 0 })
    }
    /// A read is in flight: the next one waits until it is back.
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn submit(&mut self, request: Q) -> Result<(), String> {
        if self.busy {
            return Err("A HUD read is already in flight".into());
        }
        self.input.send((self.next, request)).map_err(|_| "The HUD read worker stopped".to_owned())?;
        self.next += 1;
        self.busy = true;
        Ok(())
    }
    /// The read in flight, if it is back within `timeout`. A read sent before
    /// the last `discard_sent` comes back as None (and frees the slot).
    pub fn wait(&mut self, timeout: Duration) -> Result<Option<Done<Q, R>>, String> {
        if !self.busy {
            return Ok(None);
        }
        match self.output.recv_timeout(timeout) {
            Ok((sequence, done)) => {
                self.busy = false;
                Ok((sequence >= self.accept_from).then_some(done))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err("The HUD read worker stopped".into()),
        }
    }
    /// The backend state changed: every read already sent is out of date.
    pub fn discard_sent(&mut self) {
        self.accept_from = self.next;
    }
}

pub struct Calls<Q, R> {
    input: Sender<(u64, Q)>,
    output: Receiver<(u64, Done<Q, R>)>,
    pending: usize,
}
/// Most control actions waiting for their reply (the window also queues at most 32).
pub const MAX_PENDING: usize = 32;
impl<Q: Send + 'static, R: Send + 'static> Calls<Q, R> {
    pub fn new<F: FnMut(&Q) -> R>(name: &str, factory: impl FnOnce() -> Result<F, String> + Send + 'static) -> Result<Self, String> {
        let (input, output) = spawn(name, factory)?;
        Ok(Self { input, output, pending: 0 })
    }
    pub fn submit(&mut self, request: Q) -> Result<(), String> {
        if self.pending >= MAX_PENDING {
            return Err("Too many HUD actions waiting".into());
        }
        self.input.send((0, request)).map_err(|_| "The HUD action worker stopped".to_owned())?;
        self.pending += 1;
        Ok(())
    }
    /// The next finished action, in the order they were sent; never waits.
    pub fn take(&mut self) -> Result<Option<Done<Q, R>>, String> {
        match self.output.try_recv() {
            Ok((_, done)) => {
                self.pending -= 1;
                Ok(Some(done))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("The HUD action worker stopped".into()),
        }
    }
    #[cfg(test)]
    pub fn pending(&self) -> usize {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const SHORT: Duration = Duration::from_millis(20);
    const LONG: Duration = Duration::from_secs(2);

    /// A worker that answers only when the test releases it.
    type Step = Box<dyn FnMut(&u32) -> u32>;
    fn gated() -> (impl FnOnce() -> Result<Step, String> + Send + 'static, mpsc::Sender<()>, mpsc::Receiver<u32>) {
        let (release, gate) = mpsc::channel::<()>();
        let (started, seen) = mpsc::channel::<u32>();
        let factory = move || {
            Ok(Box::new(move |n: &u32| {
                let _ = started.send(*n);
                let _ = gate.recv();
                *n * 10
            }) as Step)
        };
        (factory, release, seen)
    }

    #[test]
    fn a_slow_read_never_blocks_the_loop_and_only_one_is_in_flight() {
        let (factory, release, seen) = gated();
        let mut reads = Reads::new("test-reads", factory).unwrap();
        reads.submit(1).unwrap();
        assert_eq!(seen.recv_timeout(LONG).unwrap(), 1);
        let began = Instant::now();
        assert!(reads.wait(SHORT).unwrap().is_none(), "the loop goes on while the read is slow");
        assert!(began.elapsed() < Duration::from_millis(500));
        assert!(reads.busy());
        assert!(reads.submit(2).is_err(), "never two reads in flight");
        release.send(()).unwrap();
        let done = reads.wait(LONG).unwrap().expect("the read comes back");
        assert_eq!((done.request, done.value), (1, 10));
        assert!(done.received >= done.sent);
        assert!(!reads.busy());
        assert!(reads.wait(SHORT).unwrap().is_none(), "nothing in flight: nothing to wait for");
    }

    #[test]
    fn a_read_sent_before_a_state_change_is_dropped() {
        let (factory, release, seen) = gated();
        let mut reads = Reads::new("test-reads", factory).unwrap();
        reads.submit(1).unwrap();
        seen.recv_timeout(LONG).unwrap();
        reads.discard_sent();
        release.send(()).unwrap();
        let end = Instant::now() + LONG;
        while reads.busy() && Instant::now() < end {
            assert!(reads.wait(SHORT).unwrap().is_none(), "the old answer is never handed out");
        }
        assert!(!reads.busy(), "the dropped read frees the slot");
        reads.submit(2).unwrap();
        release.send(()).unwrap();
        assert_eq!(reads.wait(LONG).unwrap().map(|d| d.value), Some(20), "a read sent after the change is used");
    }

    #[test]
    fn actions_keep_their_order_and_replies_and_do_not_wait_for_a_slow_read() {
        let (factory, _hold, seen) = gated();
        let mut reads = Reads::new("test-reads", factory).unwrap();
        reads.submit(7).unwrap();
        seen.recv_timeout(LONG).unwrap(); // this read never returns in the test
        let log = Arc::new(Mutex::new(Vec::new()));
        let record = log.clone();
        let mut calls = Calls::new("test-calls", move || Ok(move |q: &&'static str| {
            record.lock().unwrap().push(*q);
            q.len()
        }))
        .unwrap();
        for q in ["check", "undo", "select"] {
            calls.submit(q).unwrap();
        }
        assert_eq!(calls.pending(), 3);
        let mut replies = Vec::new();
        let end = Instant::now() + LONG;
        while replies.len() < 3 && Instant::now() < end {
            match calls.take().unwrap() {
                Some(done) => replies.push((done.request, done.value)),
                None => thread::sleep(Duration::from_millis(1)),
            }
        }
        assert_eq!(replies, [("check", 5), ("undo", 4), ("select", 6)]);
        assert_eq!(*log.lock().unwrap(), ["check", "undo", "select"]);
        assert_eq!(calls.pending(), 0);
        assert!(reads.busy(), "the read is still in flight: actions did not wait for it");
    }

    #[test]
    fn a_worker_that_cannot_start_reports_why_and_a_full_queue_refuses() {
        let failed = Reads::<u32, u32>::new("test-reads", || Err::<fn(&u32) -> u32, _>("backend down".into()));
        assert_eq!(failed.err().as_deref(), Some("backend down"));
        let (factory, _hold, _seen) = gated();
        let mut calls = Calls::new("test-calls", factory).unwrap();
        for n in 0..MAX_PENDING as u32 {
            calls.submit(n).unwrap();
        }
        assert!(calls.submit(99).is_err(), "the queue is bounded");
    }
}
