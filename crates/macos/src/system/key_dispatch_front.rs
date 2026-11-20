//! Keeps the person in front of a headless shortcut that opens a window.
//!
//! A new window opens in front of its application's other windows, and some
//! applications come forward with it. Either lands on top of what the person
//! is doing, so the watch notes where they are before the shortcut and puts
//! their window, or their application, back in front after it.

use agent_desktop_core::{
    ActionResult, AdapterError, Deadline, InteractionPolicy, KeyCombo, ProcessIdentity,
};

use crate::system::window::app_front_guard::{self, AppFrontWatch};
use crate::system::window::window_front_guard::{self, FrontWindowGuard};

const APP_LOOKS_AFTER: u32 = 30;

/// Where the person was when the shortcut was sent to process `target`: in
/// that application's key window, or in another application.
pub(super) struct FrontWatch {
    window: Option<FrontWindowGuard>,
    app: Option<AppFrontWatch>,
}

pub(super) fn watch(
    process: &ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Option<FrontWatch> {
    if policy.allow_focus_steal || !super::is_shortcut(combo) {
        return None;
    }
    let target = crate::system::process::process_identity::require_core(process)
        .ok()?
        .pid();
    let app = app_front_guard::remember()
        .filter(|person| !person.person_is_in(target))
        .map(|person| person.watch_beside(target));
    let window = if app.is_some() {
        None
    } else {
        window_front_guard::watch(target, deadline)
    };
    Some(FrontWatch { window, app })
}

/// Names what was put back as a step of the result, so the caller knows the
/// shortcut opened something that came forward.
pub(super) fn keep(
    watch: Option<FrontWatch>,
    result: Result<ActionResult, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    let mut result = result?;
    let Some(watch) = watch else {
        return Ok(result);
    };
    if watch.window.is_some_and(FrontWindowGuard::restore) {
        result
            .steps
            .push(super::semantic_step("PersonWindowKeptInFront"));
    }
    if watch.app.is_some_and(|app| app.finish(APP_LOOKS_AFTER)) {
        result
            .steps
            .push(super::semantic_step("PersonAppKeptInFront"));
    }
    Ok(result)
}
