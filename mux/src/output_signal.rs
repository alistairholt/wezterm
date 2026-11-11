//! Lets the GUI wait briefly for a pane's program to redraw after a resize.
//!
//! The parser thread for a pane (`parse_buffered_data`) reports each read
//! from the pty and each time it has applied everything it parsed to the
//! terminal model. During a live window resize the GUI takes a mark before
//! it resizes the pane, then waits on that mark before it paints, so the
//! frame it presents shows the program's redraw at the new size rather than
//! its old layout cut off at the window edge.
//!
//! The parser thread never blocks on this: it only takes a short lock to
//! record what happened and wake any waiter.

use parking_lot::{Condvar, Mutex};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the pane's output must stay quiet, after everything read so far
/// has been applied to the model, before we treat the redraw as finished.
/// While a waiter is active the parser also flushes after this much quiet
/// instead of after `mux_output_parser_coalesce_delay_ms` from the first
/// read, so a redraw that takes longer than the coalesce delay is not split
/// across two flushes.
pub const REPAINT_QUIET: Duration = Duration::from_millis(1);

/// If the program hasn't written anything this long after the resize, it is
/// probably idle (for example a shell prompt), so stop waiting.
pub const REPAINT_NO_OUTPUT: Duration = Duration::from_micros(1500);

#[derive(Default)]
struct State {
    /// Number of reads from the pty seen by the parser.
    reads: u64,
    /// When the last read happened.
    last_read: Option<Instant>,
    /// True while the parser holds parsed actions that it has not yet
    /// applied to the model, including the actions of an open
    /// synchronized output (DEC mode 2026) block.
    pending: bool,
    /// Number of synchronized output blocks that ended and were applied.
    sync_ends: u64,
    /// Number of active `RepaintWait`s.
    waiters: usize,
}

#[derive(Default)]
pub struct PaneOutputSignal {
    state: Mutex<State>,
    cond: Condvar,
}

/// Why `RepaintWait::wait` stopped waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepaintWaitOutcome {
    /// Output arrived, was applied, and then went quiet.
    Quiet,
    /// A synchronized output block ended and was applied.
    SyncEnd,
    /// Nothing arrived within `REPAINT_NO_OUTPUT` of the resize.
    NoOutput,
    /// The budget ran out.
    Budget,
}

impl PaneOutputSignal {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Called by the parser thread after each read from the pty. (Public
    /// so that tests in other crates can play the parser.)
    /// Returns true if a waiter is active, in which case the parser
    /// should flush after `REPAINT_QUIET` of quiet.
    pub fn note_read(&self) -> bool {
        let mut state = self.state.lock();
        state.reads += 1;
        state.last_read.replace(Instant::now());
        state.pending = true;
        let waiting = state.waiters > 0;
        drop(state);
        if waiting {
            self.cond.notify_all();
        }
        waiting
    }

    /// Called by the parser thread before it blocks on the next read.
    /// `pending` says whether it still holds unapplied actions;
    /// `sync_ended` says whether it applied the end of a synchronized
    /// output block since the last call.
    pub fn note_idle(&self, pending: bool, sync_ended: bool) {
        let mut state = self.state.lock();
        state.pending = pending;
        if sync_ended {
            state.sync_ends += 1;
        }
        let waiting = state.waiters > 0;
        drop(state);
        if waiting {
            self.cond.notify_all();
        }
    }

    /// Take a mark of the output seen so far. Call this just before
    /// resizing the pane, then call `wait` on the result before painting.
    pub fn begin_wait(self: &Arc<Self>) -> RepaintWait {
        let mut state = self.state.lock();
        state.waiters += 1;
        RepaintWait {
            signal: Arc::clone(self),
            reads: state.reads,
            sync_ends: state.sync_ends,
        }
    }
}

/// A mark taken by `PaneOutputSignal::begin_wait`. While it exists the
/// pane's parser flushes on quiet rather than on its coalesce timer.
pub struct RepaintWait {
    signal: Arc<PaneOutputSignal>,
    reads: u64,
    sync_ends: u64,
}

impl Drop for RepaintWait {
    fn drop(&mut self) {
        self.signal.state.lock().waiters -= 1;
    }
}

impl RepaintWait {
    /// Block until the pane's program has redrawn after the resize, or
    /// until `deadline`. `resized` is when the resize was applied.
    pub fn wait(&self, resized: Instant, deadline: Instant) -> RepaintWaitOutcome {
        let no_output_at = resized + REPAINT_NO_OUTPUT;
        let mut state = self.signal.state.lock();
        loop {
            let now = Instant::now();
            let got_output = state.reads > self.reads;

            if state.sync_ends > self.sync_ends && !state.pending {
                return RepaintWaitOutcome::SyncEnd;
            }

            let quiet_at = match (got_output, state.pending, state.last_read) {
                (true, false, Some(last)) => Some(last + REPAINT_QUIET),
                _ => None,
            };
            if let Some(quiet_at) = quiet_at {
                if now >= quiet_at {
                    return RepaintWaitOutcome::Quiet;
                }
            }

            if !got_output && now >= no_output_at {
                return RepaintWaitOutcome::NoOutput;
            }
            if now >= deadline {
                return RepaintWaitOutcome::Budget;
            }

            let mut wake = deadline;
            if !got_output {
                wake = wake.min(no_output_at);
            }
            if let Some(quiet_at) = quiet_at {
                wake = wake.min(quiet_at);
            }
            self.signal.cond.wait_until(&mut state, wake);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn no_output_stops_early() {
        let signal = PaneOutputSignal::new();
        let wait = signal.begin_wait();
        let start = Instant::now();
        let outcome = wait.wait(start, start + Duration::from_millis(50));
        assert_eq!(outcome, RepaintWaitOutcome::NoOutput);
        assert!(start.elapsed() < Duration::from_millis(40));
    }

    #[test]
    fn quiet_after_output() {
        let signal = PaneOutputSignal::new();
        let wait = signal.begin_wait();
        let start = Instant::now();
        let parser = Arc::clone(&signal);
        let thread = std::thread::spawn(move || {
            // The parser should see that a waiter is active.
            assert!(parser.note_read());
            std::thread::sleep(Duration::from_millis(1));
            parser.note_idle(false, false);
        });
        let outcome = wait.wait(start, start + Duration::from_millis(500));
        thread.join().unwrap();
        assert_eq!(outcome, RepaintWaitOutcome::Quiet);
    }

    #[test]
    fn sync_end_stops_wait() {
        let signal = PaneOutputSignal::new();
        let wait = signal.begin_wait();
        let start = Instant::now();
        signal.note_read();
        signal.note_idle(false, true);
        let outcome = wait.wait(start, start + Duration::from_millis(500));
        assert_eq!(outcome, RepaintWaitOutcome::SyncEnd);
    }

    #[test]
    fn continuous_output_hits_budget() {
        let signal = PaneOutputSignal::new();
        let wait = signal.begin_wait();
        let start = Instant::now();
        // Output is read but never fully applied, like an open
        // synchronized output block or a program that never stops.
        signal.note_read();
        let outcome = wait.wait(start, start + Duration::from_millis(5));
        assert_eq!(outcome, RepaintWaitOutcome::Budget);
    }

    #[test]
    fn dropping_wait_clears_waiter() {
        let signal = PaneOutputSignal::new();
        let wait = signal.begin_wait();
        assert!(signal.note_read());
        drop(wait);
        assert!(!signal.note_read());
    }
}
