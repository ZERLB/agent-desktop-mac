//! Moves an application's keyboard between its windows and back, observing
//! each move through accessibility and repeating it until it holds.

use agent_desktop_core::{AdapterError, Deadline};
use std::time::Duration;

use super::window_key_handoff as handoff;
use crate::tree::AXElement;

const OBSERVE_STEP: Duration = Duration::from_millis(15);
const OBSERVE_ATTEMPTS: u32 = 10;
const HAND_OVER_SETTLES: [Duration; 5] = [
    Duration::from_millis(10),
    Duration::from_millis(25),
    Duration::from_millis(60),
    Duration::from_millis(120),
    Duration::from_millis(200),
];
pub(super) const CLAIM_ATTEMPTS: usize = 3;
const HAND_BACK_BUDGET_MS: u64 = 3_000;
const PERSON_IDLE: Duration = Duration::from_millis(800);
const PERSON_WAIT_STEP: Duration = Duration::from_millis(100);
const PERSON_WAIT_ATTEMPTS: u32 = 20;

/// One move of the keyboard of process `pid` to `window`, numbered `to`.
pub(super) struct Move<'a> {
    pub(super) pid: i32,
    pub(super) app: &'a AXElement,
    pub(super) from: Option<u32>,
    pub(super) to: u32,
    pub(super) window: &'a AXElement,
}

/// What to undo when a claim ends: the window that held the keyboard gets it
/// back, and a background application is left without a focused window, the
/// way it rests, so its pages do not believe the person is looking at them.
pub(super) struct HandBack {
    pub(super) pid: i32,
    pub(super) app: AXElement,
    pub(super) claimed: u32,
    pub(super) previous: Option<(u32, AXElement)>,
    pub(super) background: bool,
}

impl HandBack {
    pub(super) fn run(self) {
        let deadline = Deadline::detached_after(HAND_BACK_BUDGET_MS).ok();
        let restored = self.previous.as_ref().is_none_or(|(number, window)| {
            let step = Move {
                pid: self.pid,
                app: &self.app,
                from: Some(self.claimed),
                to: *number,
                window,
            };
            match deadline {
                Some(deadline) => switch(step, deadline, HAND_OVER_SETTLES.len()),
                None => handoff::hand_over(step.pid, step.from, step.to, HAND_OVER_SETTLES[0]),
            }
        });
        if self.rests_in_background() {
            let resting = match (&self.previous, restored) {
                (Some((number, _)), true) => *number,
                _ => self.claimed,
            };
            rest(self.pid, &self.app, resting, deadline);
        }
        tracing::debug!(pid = self.pid, restored, "window: keyboard handed back");
    }

    /// The person can bring the application forward or leave it while the
    /// claim lives, so where it rests is read when the claim ends.
    fn rests_in_background(&self) -> bool {
        handoff::is_front_process(self.pid).map_or(self.background, |front| !front)
    }
}

/// Posts the hand-over until accessibility reports `window` as the key
/// window. An application that is busy can handle the loss and the gain
/// together and ignore them, so each attempt separates them further.
pub(super) fn switch(step: Move<'_>, deadline: Deadline, attempts: usize) -> bool {
    for settle in HAND_OVER_SETTLES.into_iter().take(attempts) {
        if !handoff::hand_over(step.pid, step.from, step.to, settle) {
            return false;
        }
        for _ in 0..OBSERVE_ATTEMPTS {
            if holds_keyboard(step.app, step.window, deadline) {
                return true;
            }
            let Ok(pause) = deadline.remaining_slice(OBSERVE_STEP) else {
                return false;
            };
            std::thread::sleep(pause);
        }
    }
    false
}

/// Gives the focus to window `number`, which already holds the keyboard of a
/// background application, so the application handles keys as the active one.
/// The application takes the records in its own time, and a web runtime that
/// is asked to focus before it has would bring the application forward, so
/// this returns once the application answers that it is active. An
/// application that never does is put back to rest.
pub(super) fn focus(pid: i32, app: &AXElement, number: u32, deadline: Deadline) -> bool {
    for settle in HAND_OVER_SETTLES.into_iter().take(CLAIM_ATTEMPTS) {
        if !handoff::hand_over(pid, None, number, settle) {
            return false;
        }
        for _ in 0..OBSERVE_ATTEMPTS {
            if crate::tree::copy_bool_attr(app, "AXFrontmost", deadline) == Some(true) {
                return true;
            }
            let Ok(pause) = deadline.remaining_slice(OBSERVE_STEP) else {
                break;
            };
            std::thread::sleep(pause);
        }
    }
    rest(pid, app, number, Some(deadline));
    false
}

/// Puts a background application back to rest: window `number` keeps the
/// keyboard but loses the focus. An application that is still handling the
/// keys can miss the record, so it is repeated until the application answers
/// that it is no longer active.
fn rest(pid: i32, app: &AXElement, number: u32, deadline: Option<Deadline>) {
    for settle in HAND_OVER_SETTLES {
        handoff::release(pid, number);
        let Some(deadline) = deadline else {
            return;
        };
        for _ in 0..OBSERVE_ATTEMPTS {
            if crate::tree::copy_bool_attr(app, "AXFrontmost", deadline) != Some(true) {
                return;
            }
            std::thread::sleep(OBSERVE_STEP);
        }
        std::thread::sleep(settle);
    }
}

/// Waits for the person to stop using the physical keyboard and mouse
/// buttons, because their next key would follow the keyboard into the
/// claimed window.
pub(super) fn person_went_idle(deadline: Deadline) -> bool {
    for _ in 0..PERSON_WAIT_ATTEMPTS {
        if handoff::person_idle_for().is_some_and(|idle| idle >= PERSON_IDLE) {
            return true;
        }
        let Ok(pause) = deadline.remaining_slice(PERSON_WAIT_STEP) else {
            return false;
        };
        std::thread::sleep(pause);
    }
    false
}

pub(super) fn key_window(
    app: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    match element_attribute(app, "AXFocusedWindow", deadline)? {
        Some(holder) => Ok(Some(holder)),
        None => element_attribute(app, "AXMainWindow", deadline),
    }
}

pub(super) fn element_attribute(
    source: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result =
        crate::tree::properties::attributes::copy_element_attr_result(source, attribute, deadline);
    deadline.ensure_remaining()?;
    Ok(result.ok().flatten())
}

fn holds_keyboard(app: &AXElement, window: &AXElement, deadline: Deadline) -> bool {
    key_window(app, deadline)
        .ok()
        .flatten()
        .is_some_and(|holder| crate::tree::properties::capabilities::same_element(window, &holder))
}
