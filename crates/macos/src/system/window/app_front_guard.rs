//! Keeps the person's application in front when a headless action makes a
//! background application come forward.
//!
//! An application can bring itself forward when it opens a window. A browser
//! built on Chromium does for every new window, and then stays in front of
//! the person's work, taking their keys, until they click their own window
//! again. The guard remembers which application was in front before the
//! action, watches beside the action from another thread, and puts that
//! application back as soon as the target takes its place.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::window_key_handoff as handoff;
use crate::system::process::process_identity::ProcessIdentity;

const WATCH_STEP: Duration = Duration::from_millis(10);
const RESTORE_STEP: Duration = Duration::from_millis(5);
const SWITCH_ATTEMPTS: u32 = 40;
const ACTIVATION_ATTEMPTS: u32 = 300;
const MAX_RESTORES: u32 = 3;
const STILL_ACTING: i32 = -1;

/// The application the person had in front when the watch began, and its
/// front window.
pub(crate) struct AppFrontGuard {
    person: ProcessIdentity,
    window: Option<u32>,
}

/// A watch running beside an action. Dropping it ends the watch at once.
pub(crate) struct AppFrontWatch {
    looks_left: Arc<AtomicI32>,
    watcher: Option<JoinHandle<bool>>,
}

#[derive(Debug, PartialEq, Eq)]
enum Standing {
    Kept,
    Taken,
    Elsewhere,
}

/// Remembers the application in front. `None` when this system cannot tell
/// which one it is.
pub(crate) fn remember() -> Option<AppFrontGuard> {
    let person = ProcessIdentity::capture(handoff::front_pid()?)
        .ok()
        .flatten()?;
    let window = super::cg_window_cover::front_window_of(person.pid());
    Some(AppFrontGuard { person, window })
}

impl AppFrontGuard {
    /// Whether the person is in process `pid` itself, which then takes
    /// nothing from them by coming forward.
    pub(crate) fn person_is_in(&self, pid: i32) -> bool {
        self.person.pid() == pid
    }

    /// Watches for process `taker` in front of the person's application
    /// until the watch is finished or dropped.
    pub(crate) fn watch_beside(self, taker: i32) -> AppFrontWatch {
        let looks_left = Arc::new(AtomicI32::new(STILL_ACTING));
        let signal = Arc::clone(&looks_left);
        let watcher = std::thread::Builder::new()
            .name("front-watch".into())
            .spawn(move || self.watch(taker, &signal))
            .ok();
        AppFrontWatch {
            looks_left,
            watcher,
        }
    }

    fn watch(&self, taker: i32, signal: &AtomicI32) -> bool {
        let (mut restores, mut looks_after) = (0, 0);
        loop {
            match standing(handoff::front_pid(), self.person.pid(), taker) {
                Standing::Taken if restores < MAX_RESTORES && self.bring_person_forward() => {
                    restores += 1;
                }
                Standing::Kept => {}
                Standing::Taken | Standing::Elsewhere => return restores > 0,
            }
            let allowed = signal.load(Ordering::Acquire);
            if allowed != STILL_ACTING {
                if looks_after >= allowed {
                    return restores > 0;
                }
                looks_after += 1;
            }
            std::thread::sleep(WATCH_STEP);
        }
    }

    /// The window server's own switch is immediate. The public activation
    /// request is the fallback: the system may take a second to grant it.
    fn bring_person_forward(&self) -> bool {
        let started = Instant::now();
        let switched = self
            .window
            .is_some_and(|window| handoff::bring_forward(self.person.pid(), window))
            && self.came_back(SWITCH_ATTEMPTS);
        let back = switched || (self.ask_activation() && self.came_back(ACTIVATION_ATTEMPTS));
        tracing::debug!(
            pid = self.person.pid(),
            switched,
            back,
            elapsed_ms = started.elapsed().as_millis(),
            "system: put the person's application back in front"
        );
        back
    }

    fn ask_activation(&self) -> bool {
        crate::system::cocoa_runtime::ensure_cocoa_multithreaded()
            .and_then(|()| crate::system::focus_bridge::activate(self.person))
            .is_ok()
    }

    fn came_back(&self, attempts: u32) -> bool {
        for _ in 0..attempts {
            if handoff::front_pid() == Some(self.person.pid()) {
                return true;
            }
            std::thread::sleep(RESTORE_STEP);
        }
        false
    }
}

impl AppFrontWatch {
    /// Keeps looking `looks` more times, a step apart, for an application
    /// that comes forward after the action returned. Returns whether the
    /// person's application had to be put back and was.
    pub(crate) fn finish(mut self, looks: u32) -> bool {
        self.end(i32::try_from(looks).unwrap_or(i32::MAX))
    }

    fn end(&mut self, looks: i32) -> bool {
        self.looks_left.store(looks, Ordering::Release);
        self.watcher
            .take()
            .and_then(|watcher| watcher.join().ok())
            .unwrap_or(false)
    }
}

impl Drop for AppFrontWatch {
    fn drop(&mut self) {
        self.end(0);
    }
}

/// A third application in front is the person's own move, and an unknown
/// front is no ground to move anything.
fn standing(front: Option<i32>, person: i32, taker: i32) -> Standing {
    match front {
        Some(front) if front == taker => Standing::Taken,
        Some(front) if front == person => Standing::Kept,
        _ => Standing::Elsewhere,
    }
}

#[cfg(test)]
#[path = "app_front_guard_tests.rs"]
mod tests;
