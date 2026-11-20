//! Keeps the person in front of a headless action that opens a window or
//! makes its application come forward.
//!
//! A new window opens in front of its application's other windows, and some
//! applications come forward with it, or when their focus is written. Either
//! lands on top of what the person is doing, so the watch notes where they
//! are before the action and puts their window, or their application, back in
//! front.

use agent_desktop_core::{ActionResult, ActionStep, AdapterError, Deadline, StepMechanism};

use super::app_front_guard::{self, AppFrontWatch};
use super::window_front_guard::{self, FrontWindowGuard};

/// Where the person was when the action began: in the target application's
/// key window, or in another application.
pub(crate) struct PersonFront {
    window: Option<FrontWindowGuard>,
    app: Option<AppFrontWatch>,
}

/// How many more looks each watch is given once the action returned. A
/// window watch with none is not begun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Patience {
    pub(crate) window_looks: u32,
    pub(crate) app_looks: u32,
}

/// Begins the watch for an action sent to process `target`.
pub(crate) fn watch(target: i32, patience: Patience, deadline: Deadline) -> PersonFront {
    let app = app_front_guard::remember()
        .filter(|person| !person.person_is_in(target))
        .map(|person| person.watch_beside(target));
    let window = if app.is_some() || patience.window_looks == 0 {
        None
    } else {
        window_front_guard::watch(target, deadline)
    };
    PersonFront { window, app }
}

impl PersonFront {
    /// Ends the watch and names what was put back as a step of the result,
    /// so the caller knows the action opened something that came forward.
    pub(crate) fn keep(
        self,
        patience: Patience,
        result: Result<ActionResult, AdapterError>,
    ) -> Result<ActionResult, AdapterError> {
        let mut result = result?;
        if self
            .window
            .is_some_and(|window| window.restore(patience.window_looks))
        {
            result.steps.push(step("PersonWindowKeptInFront"));
        }
        if self.app.is_some_and(|app| app.finish(patience.app_looks)) {
            result.steps.push(step("PersonAppKeptInFront"));
        }
        Ok(result)
    }
}

fn step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}
