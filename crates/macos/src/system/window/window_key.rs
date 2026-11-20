//! The window of an application that receives keys posted to its process.
//!
//! Process-targeted keys reach the application's key window even while the
//! application is in the background, so a key meant for another window of the
//! same application would land in the wrong one. A background application lets
//! its key window change without being activated. The frontmost application
//! does not, because its key window is the one the person is working in.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use std::time::Duration;

use crate::tree::AXElement;

const SWITCH_STEP: Duration = Duration::from_millis(20);
const SWITCH_ATTEMPTS: u32 = 25;

/// How a window stands toward its application's keyboard after a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyClaim {
    AlreadyKey,
    MadeKey,
    Refused(&'static str),
}

impl KeyClaim {
    pub(crate) fn made_key(self) -> bool {
        self == Self::MadeKey
    }
}

/// Makes `window` the window that receives keys posted to process `pid`.
///
/// A window that already holds the keyboard is left alone. Otherwise the
/// window becomes its application's main window, which does not activate the
/// application, only while the application is in the background, visible and
/// the window is not minimized. Every other standing is refused with its cause
/// and nothing is changed.
pub(crate) fn claim(
    window: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyClaim, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    if holds_keyboard(&app, window, deadline)? {
        return Ok(KeyClaim::AlreadyKey);
    }
    if let Some(cause) = refusal_cause(
        flag(&app, "AXFrontmost", deadline),
        flag(&app, "AXHidden", deadline),
        flag(window, "AXMinimized", deadline),
    ) {
        return Ok(KeyClaim::Refused(cause));
    }
    if crate::actions::ax_helpers::set_ax_bool_or_err(window, "AXMain", true, deadline).is_err() {
        return Ok(KeyClaim::Refused("main_window_write_refused"));
    }
    for _ in 0..SWITCH_ATTEMPTS {
        if holds_keyboard(&app, window, deadline)? {
            return Ok(KeyClaim::MadeKey);
        }
        std::thread::sleep(deadline.remaining_slice(SWITCH_STEP)?);
    }
    Ok(KeyClaim::Refused("main_window_not_observed"))
}

/// Claims the keyboard for the window that holds `element`. An element with
/// no window, such as a menu bar item, has no key window to claim.
pub(crate) fn claim_for_element(
    element: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyClaim, AdapterError> {
    match element_attribute(element, "AXWindow", deadline)? {
        Some(window) => claim(&window, pid, deadline),
        None => Ok(KeyClaim::Refused("element_has_no_window")),
    }
}

/// The error for a key that was not sent because another window of the same
/// application holds the keyboard.
pub(crate) fn refusal(cause: &'static str, window_id: Option<&str>) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Another window of this application holds the keyboard, so no key was sent",
    )
    .with_details(serde_json::json!({
        "kind": "window_not_key",
        "check": "window_active",
        "cause": cause,
        "window_id": window_id,
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .with_suggestion(
        "Keys posted to an application reach its key window. Retry while the application is in the \
         background, where this window can take the keyboard without coming forward, or retry with \
         --headed to bring this window forward.",
    )
}

/// Why a window that does not hold the keyboard must not take it, or `None`
/// when it can take it without disturbing the person.
fn refusal_cause(
    app_frontmost: bool,
    app_hidden: bool,
    window_minimized: bool,
) -> Option<&'static str> {
    if app_frontmost {
        Some("sibling_window_holds_keyboard")
    } else if app_hidden {
        Some("application_hidden")
    } else if window_minimized {
        Some("window_minimized")
    } else {
        None
    }
}

fn holds_keyboard(
    app: &AXElement,
    window: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let key_window = match element_attribute(app, "AXFocusedWindow", deadline)? {
        Some(key_window) => Some(key_window),
        None => element_attribute(app, "AXMainWindow", deadline)?,
    };
    Ok(key_window.is_some_and(|key_window| {
        crate::tree::properties::capabilities::same_element(window, &key_window)
    }))
}

fn element_attribute(
    source: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result =
        crate::tree::properties::attributes::copy_element_attr_result(source, attribute, deadline);
    deadline.ensure_remaining()?;
    Ok(result.ok().flatten())
}

fn flag(element: &AXElement, attribute: &str, deadline: Deadline) -> bool {
    crate::tree::copy_bool_attr(element, attribute, deadline) == Some(true)
}

#[cfg(test)]
#[path = "window_key_tests.rs"]
mod tests;
