//! Keeps the person's window in front when a headless shortcut opens a new
//! window in the application they are using.
//!
//! A new window opens in front of its application's other windows and takes
//! the keyboard. In a background application that stays behind the person's
//! work. In the application the person is in, it would cover the window they
//! are using and take their next keys, so their window is put back in front
//! and the new window is left behind it.

use agent_desktop_core::Deadline;
use std::time::Duration;

use super::window_key_handoff as handoff;
use super::window_key_switch::key_window;
use crate::tree::AXElement;

const WATCH_STEP: Duration = Duration::from_millis(40);
const RESTORE_ATTEMPTS: u32 = 10;
const BUDGET_MS: u64 = 2_000;
const MAX_KNOWN_WINDOWS: usize = 64;

/// The window the person is in and the windows their application had when
/// the watch began.
pub(crate) struct FrontWindowGuard {
    app: AXElement,
    person: AXElement,
    known: Vec<AXElement>,
}

/// Begins a watch when process `pid` is the application in front and has a
/// key window. Any other application opens its windows behind the person.
pub(crate) fn watch(pid: i32, deadline: Deadline) -> Option<FrontWindowGuard> {
    if handoff::is_front_process(pid) != Some(true) {
        return None;
    }
    let app = crate::tree::element_for_pid(pid);
    let person = key_window(&app, deadline).ok().flatten()?;
    let known = crate::tree::properties::attributes::copy_ax_array_prefix_result(
        &app,
        "AXWindows",
        MAX_KNOWN_WINDOWS,
        deadline,
    )
    .ok()
    .flatten()?;
    Some(FrontWindowGuard { app, person, known })
}

impl FrontWindowGuard {
    /// Looks `looks` times, a step apart, for a window that did not exist in
    /// front and puts the person's window back in front of it. Returns
    /// whether it did.
    pub(crate) fn restore(self, looks: u32) -> bool {
        let Ok(deadline) = Deadline::detached_after(BUDGET_MS) else {
            return false;
        };
        for _ in 0..looks {
            if self.new_window_took_the_front(deadline) {
                return self.bring_person_forward(deadline);
            }
            std::thread::sleep(WATCH_STEP);
        }
        false
    }

    fn new_window_took_the_front(&self, deadline: Deadline) -> bool {
        let Some(front) = key_window(&self.app, deadline).ok().flatten() else {
            return false;
        };
        let known = self
            .known
            .iter()
            .any(|window| crate::tree::properties::capabilities::same_element(window, &front));
        is_new_standard_window(known, subrole(&front, deadline).as_deref())
    }

    fn bring_person_forward(&self, deadline: Deadline) -> bool {
        let _ =
            crate::actions::ax_helpers::set_ax_bool_or_err(&self.person, "AXMain", true, deadline);
        for _ in 0..RESTORE_ATTEMPTS {
            let back = key_window(&self.app, deadline)
                .ok()
                .flatten()
                .is_some_and(|front| {
                    crate::tree::properties::capabilities::same_element(&self.person, &front)
                });
            if back {
                return true;
            }
            std::thread::sleep(WATCH_STEP);
        }
        false
    }
}

/// Only a standard window that the application did not have counts: a dialog
/// or sheet belongs to the window it came from and must stay where it is.
fn is_new_standard_window(known: bool, subrole: Option<&str>) -> bool {
    !known && subrole == Some("AXStandardWindow")
}

fn subrole(window: &AXElement, deadline: Deadline) -> Option<String> {
    crate::tree::properties::attributes::copy_string_attr_result(window, "AXSubrole", deadline)
        .ok()
        .flatten()
}

#[cfg(test)]
#[path = "window_front_guard_tests.rs"]
mod tests;
