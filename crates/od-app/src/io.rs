//! A background worker for file work, so a big drawing, a slow disk or a
//! network path never freezes the window.
//!
//! Work is a closure that runs on one worker thread, in the order it was
//! submitted (so a save can never be overtaken by the clean-up after it), and
//! returns a [`Done`]. The UI thread collects those each frame with
//! `AppState::poll_io` and applies them to the drawings; the worker never
//! touches application state, only the copies it was handed.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use od_files::{FileError, Loaded};

/// What a finished job reports back.
pub enum Done {
    Opened {
        path: PathBuf,
        /// Boxed: a loaded drawing is far larger than the other results.
        result: Box<Result<Loaded, FileError>>,
    },
    Saved {
        uid: u64,
        path: PathBuf,
        /// Where the history stood when the copy was taken, so edits made
        /// while the file was being written still count as unsaved.
        position: u64,
        result: Result<Vec<String>, FileError>,
    },
    Exported {
        path: PathBuf,
        result: Result<Vec<String>, FileError>,
    },
    /// One pass of recovery copies: for each drawing written, where its
    /// history stood and whether it worked.
    Autosaved(Vec<(u64, u64, std::io::Result<()>)>),
    /// Nothing to report (a recovery copy was removed).
    Finished,
}

type Job = Box<dyn FnOnce() -> Done + Send>;
type Waker = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;
type Results = Sender<(u64, Done)>;

/// A job that has not been collected yet.
struct Running {
    id: u64,
    since: Instant,
    /// Set to abandon it; only jobs that can be given up have one.
    cancel: Option<Arc<AtomicBool>>,
}

enum Mode {
    /// Threads of their own.
    Background {
        /// The queue of the ordered worker, started when first needed.
        ordered: Option<Sender<(u64, Job)>>,
    },
    /// Run each job on the spot. For tests and headless use.
    #[cfg_attr(not(test), allow(dead_code))]
    Inline,
}

/// Two kinds of job. *Ordered* jobs share one worker and run in the order
/// submitted (recovery copies: a copy must never be deleted before the write
/// that made it has finished). *Cancellable* jobs, the ones the user waits for
/// (open, save, export), each get a thread of their own: one stuck on a slow
/// disk or network path then blocks nothing else, and can be given up on.
/// A thread cannot be stopped from outside, so giving up means ignoring its
/// result and, for a save, making sure it never replaces the file.
pub struct Io {
    mode: Mode,
    done: Receiver<(u64, Done)>,
    done_sender: Results,
    live: Vec<Running>,
    next_id: u64,
    wake: Waker,
}

impl Io {
    pub fn background() -> Self {
        Self::with(Mode::Background { ordered: None })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn inline() -> Self {
        Self::with(Mode::Inline)
    }

    fn with(mode: Mode) -> Self {
        let (done_sender, done) = channel();
        Self {
            mode,
            done,
            done_sender,
            live: Vec::new(),
            next_id: 0,
            wake: Arc::default(),
        }
    }

    /// Called from a worker whenever a job finishes, to get the UI to run a
    /// frame and collect it (the app only draws when something happens).
    pub fn set_waker(&self, wake: impl Fn() + Send + 'static) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(Box::new(wake));
        }
    }

    pub fn is_inline(&self) -> bool {
        matches!(self.mode, Mode::Inline)
    }

    /// Whether any job has not been collected (or given up on) yet.
    pub fn busy(&self) -> bool {
        !self.live.is_empty()
    }

    /// Whether a job the user could cancel has been running for `after`.
    pub fn stalled(&self, after: Duration) -> bool {
        self.live
            .iter()
            .any(|r| r.cancel.is_some() && r.since.elapsed() >= after)
    }

    fn start(&mut self, cancel: Option<Arc<AtomicBool>>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.live.push(Running {
            id,
            since: Instant::now(),
            cancel,
        });
        id
    }

    fn drop_running(&mut self, id: u64) {
        self.live.retain(|r| r.id != id);
    }

    /// Run `job` after every earlier ordered job.
    pub fn submit(&mut self, job: impl FnOnce() -> Done + Send + 'static) {
        let id = self.start(None);
        match &mut self.mode {
            Mode::Inline => {
                let _ = self.done_sender.send((id, job()));
            }
            Mode::Background { ordered } => {
                let queue = ordered.get_or_insert_with(|| {
                    spawn_ordered(self.done_sender.clone(), self.wake.clone())
                });
                // A dead worker (it panicked) loses the job: do not wait for
                // a result that will never come.
                if queue.send((id, Box::new(job))).is_err() {
                    self.drop_running(id);
                }
            }
        }
    }

    /// Run `job` on a thread of its own. It is handed a flag that is set if
    /// the user gives up, and should stop what it safely can.
    pub fn submit_cancellable(
        &mut self,
        job: impl FnOnce(&AtomicBool) -> Done + Send + 'static,
    ) -> u64 {
        let cancel = Arc::new(AtomicBool::new(false));
        let id = self.start(Some(cancel.clone()));
        match &self.mode {
            Mode::Inline => {
                let _ = self.done_sender.send((id, job(&cancel)));
            }
            Mode::Background { .. } => {
                let results = self.done_sender.clone();
                let wake = self.wake.clone();
                let spawned = thread::Builder::new()
                    .name("od-files-job".into())
                    .spawn(move || {
                        let done = job(&cancel);
                        deliver(&results, &wake, id, done);
                    });
                if spawned.is_err() {
                    self.drop_running(id);
                }
            }
        }
        id
    }

    /// Give up on one cancellable job (by the id `submit_cancellable` gave).
    /// Its result is thrown away when it arrives. False if it was not
    /// running or cannot be given up.
    pub fn cancel_job(&mut self, id: u64) -> bool {
        let Some(at) = self.live.iter().position(|r| r.id == id) else {
            return false;
        };
        let Some(flag) = self.live[at].cancel.clone() else {
            return false;
        };
        flag.store(true, Ordering::Relaxed);
        self.live.remove(at);
        true
    }

    /// Give up on every cancellable job. Their results are thrown away when
    /// they arrive. Returns how many there were.
    pub fn cancel_all(&mut self) -> usize {
        let before = self.live.len();
        self.live.retain(|r| match &r.cancel {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                false
            }
            None => true,
        });
        before - self.live.len()
    }

    /// The next finished job that was not given up on. Never blocks.
    pub fn next(&mut self) -> Option<Done> {
        loop {
            let (id, done) = self.done.try_recv().ok()?;
            if self.live.iter().any(|r| r.id == id) {
                self.drop_running(id);
                return Some(done);
            }
        }
    }

    /// Wait for the next finished job (tests).
    #[cfg(test)]
    pub fn wait_next(&mut self) -> Option<Done> {
        while self.busy() {
            let (id, done) = self.done.recv_timeout(Duration::from_secs(20)).ok()?;
            if self.live.iter().any(|r| r.id == id) {
                self.drop_running(id);
                return Some(done);
            }
        }
        None
    }
}

/// Hand a result to the UI thread and wake it.
fn deliver(results: &Results, wake: &Waker, id: u64, done: Done) {
    if results.send((id, done)).is_ok() {
        if let Ok(slot) = wake.lock() {
            if let Some(wake) = slot.as_ref() {
                wake();
            }
        }
    }
}

fn spawn_ordered(results: Results, wake: Waker) -> Sender<(u64, Job)> {
    let (jobs, queue) = channel::<(u64, Job)>();
    let started = thread::Builder::new()
        .name("od-files".into())
        .spawn(move || {
            for (id, job) in queue {
                deliver(&results, &wake, id, job());
            }
        });
    if started.is_err() {
        // No thread: the sender below has no receiver and `submit` notices.
        let (dead, _) = channel();
        return dead;
    }
    jobs
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn jobs_finish_in_the_order_they_were_submitted() {
        let mut io = Io::background();
        for n in 0..5u64 {
            io.submit(move || {
                // Earlier jobs are slower: order must still hold.
                thread::sleep(Duration::from_millis(5 * (5 - n)));
                Done::Autosaved(vec![(n, 0, Ok(()))])
            });
        }
        let mut seen = Vec::new();
        while let Some(Done::Autosaved(items)) = io.wait_next() {
            seen.push(items[0].0);
        }
        assert_eq!(seen, [0, 1, 2, 3, 4]);
        assert!(!io.busy());
    }

    #[test]
    fn busy_until_collected_and_the_waker_is_called() {
        let mut io = Io::background();
        let (woken, heard) = channel();
        io.set_waker(move || {
            let _ = woken.send(());
        });
        io.submit(|| Done::Finished);
        assert!(io.busy());
        heard.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(io.wait_next(), Some(Done::Finished)));
        assert!(!io.busy());
    }

    #[test]
    fn a_cancelled_job_is_forgotten_and_blocks_nothing() {
        let mut io = Io::background();
        let (release, held) = channel::<()>();
        io.submit_cancellable(move |cancelled| {
            // Stuck, as on a dead network path, until the test lets go.
            let _ = held.recv_timeout(Duration::from_secs(10));
            assert!(cancelled.load(Ordering::Relaxed), "told to give up");
            Done::Finished
        });
        assert!(io.busy());
        assert!(!io.stalled(Duration::from_secs(60)));
        assert!(io.stalled(Duration::ZERO));
        assert_eq!(io.cancel_all(), 1);
        assert!(!io.busy(), "nothing is waited for any more");

        // Later work runs although the stuck job has not returned.
        io.submit_cancellable(|_| Done::Autosaved(vec![(7, 0, Ok(()))]));
        assert!(matches!(io.wait_next(), Some(Done::Autosaved(_))));
        // And once the stuck job does return, its result is thrown away.
        drop(release);
        io.submit(|| Done::Autosaved(vec![(8, 0, Ok(()))]));
        match io.wait_next() {
            Some(Done::Autosaved(items)) => assert_eq!(items[0].0, 8),
            _ => panic!("expected the later job, not the cancelled one"),
        }
    }

    #[test]
    fn ordered_jobs_cannot_be_cancelled() {
        let mut io = Io::background();
        io.submit(|| Done::Finished);
        assert_eq!(io.cancel_all(), 0);
        assert!(matches!(io.wait_next(), Some(Done::Finished)));
    }

    #[test]
    fn inline_runs_at_once() {
        let mut io = Io::inline();
        io.submit(|| Done::Finished);
        assert!(matches!(io.next(), Some(Done::Finished)));
    }
}
