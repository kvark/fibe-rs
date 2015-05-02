//! Back-end module for the task queue. The back-end is running
//! on a separate thread. All it does is listening to a command
//! channel and starting new tasks when the time comes.

use std::boxed::FnBox;
use std::sync::Mutex;
use std::thread;
use pulse::*;

use {Handle, Wait};

struct Inner {
    exit: Option<Pulse>,
    exit_method: Wait,
    active: SelectMap<thread::JoinHandle<()>>
}

/// Task queue back-end.
pub struct Backend {
    inner: Mutex<Inner>
}

impl Backend {
    /// Create a new back-end.
    pub fn new() -> Backend {
        Backend {
            inner: Mutex::new(Inner{
                exit: None,
                exit_method: Wait::None,
                active: SelectMap::new(),
            })
        }
    }

    pub fn start(&self, mut deps: Vec<Handle>, task: Box<FnBox() + Send>) -> Handle {
        let (signal, complete) = Signal::new();

        let pulse = if deps.len() == 0 {
            let (pulse, t) = Signal::new();
            t.pulse();
            pulse
        } else if deps.len() == 1 {
            // If only one, we can just use the handle in it's raw form
            deps.pop().unwrap()
        } else {
            let barrier = Barrier::new(deps);
            barrier.signal()
        };

        pulse.callback(move || {
            thread::spawn(move || {
                task();
                complete.pulse();
            });
        });
        signal
    }

    pub fn exit(&self, wait: Wait) {
        let mut guard = self.inner.lock().unwrap();
        guard.exit_method = wait;
        let t = guard.exit.take().unwrap();
        t.pulse();
    }

    pub fn run(&self, ack: Pulse) {
        let (exit_p, exit) = Signal::new();
        let mut select = Select::new();
        let exit_id = select.add(exit_p);
        let mut active_id = {
            let mut guard = self.inner.lock().unwrap();
            guard.exit = Some(exit);
            select.add(guard.active.signal())
        };

        // Tell the caller that we have started
        ack.pulse();

        let mut exit_method = None;
        while let Some(pulse) = select.next() {
            let mut guard = self.inner.lock().unwrap();

            if pulse.id() == active_id {
                active_id = select.add(guard.active.signal());
                if let Some((_, task)) = guard.active.try_next() {
                    task.join().unwrap();
                };
            } else if exit_id == pulse.id() {
                exit_method = Some(guard.exit_method);
            }

            match (exit_method, guard.active.len()) {
                (Some(Wait::None), _) => break,
                (Some(Wait::Active), 0) => break,
                (Some(Wait::Pending), 0) => break,
                _ => ()
            }
        }
    }
}
