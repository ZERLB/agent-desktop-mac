//! The window of an application that receives keys posted to its process.
//!
//! Process-targeted keys reach the application's key window, in the background
//! and in front alike, so a key meant for another window of the same
//! application would land in the wrong one. A claim hands the keyboard to the
//! target window without activating the application or reordering its
//! windows, and hands it back when the claim is dropped, so the window the
//! person was in holds the keyboard again once the keys are queued.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use super::window_key_handoff as handoff;
use super::window_key_switch::{self as switch, HandBack, Move, element_attribute, key_window};
use crate::tree::AXElement;

const HAND_OVER_SWITCH: &str = "AGENT_DESKTOP_KEY_HANDOFF";

/// How a window stands toward its application's keyboard after a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyClaim {
    AlreadyKey,
    HandedOver,
    Refused(&'static str),
}

/// A claim on an application's keyboard. Dropping it hands the keyboard back
/// to the window that held it.
pub(crate) struct KeyboardClaim {
    standing: KeyClaim,
    hand_back: Option<HandBack>,
}

impl KeyboardClaim {
    fn settled(standing: KeyClaim) -> Self {
        Self {
            standing,
            hand_back: None,
        }
    }

    pub(crate) fn refusal(&self) -> Option<&'static str> {
        match self.standing {
            KeyClaim::Refused(cause) => Some(cause),
            KeyClaim::AlreadyKey | KeyClaim::HandedOver => None,
        }
    }

    pub(crate) fn handed_over(&self) -> bool {
        self.standing == KeyClaim::HandedOver
    }
}

impl Drop for KeyboardClaim {
    fn drop(&mut self) {
        if let Some(back) = self.hand_back.take() {
            back.run();
        }
    }
}

/// Gives `window` the keyboard of process `pid` for as long as the claim lives.
///
/// A window that already holds the keyboard is left alone, and so is a
/// transient window of a background application, such as a dialog the caller's
/// own action opened. A hidden application, a minimized window and a person
/// who is typing in the frontmost application are refused with their cause,
/// and nothing changes.
pub(crate) fn claim(
    window: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyboardClaim, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    let holder = key_window(&app, deadline)?;
    if holder
        .as_ref()
        .is_some_and(|holder| crate::tree::properties::capabilities::same_element(window, holder))
    {
        return Ok(KeyboardClaim::settled(KeyClaim::AlreadyKey));
    }
    let frontmost = in_front(&app, pid, deadline);
    let standing = Standing {
        frontmost,
        hidden: flag(&app, "AXHidden", deadline),
        minimized: flag(window, "AXMinimized", deadline),
        holder_is_transient: holder
            .as_ref()
            .is_some_and(|holder| is_transient(holder, deadline)),
        hand_over_offered: hand_over_offered(),
    };
    match route(standing) {
        Route::Leave => Ok(KeyboardClaim::settled(KeyClaim::AlreadyKey)),
        Route::Refuse(cause) => Ok(KeyboardClaim::settled(KeyClaim::Refused(cause))),
        Route::HandOver => hand_over(
            Parties {
                app: &app,
                window,
                holder: holder.as_ref(),
                pid,
            },
            frontmost,
            deadline,
        ),
    }
}

/// Claims the keyboard for the window that holds `element`. An element with
/// no window, such as a menu bar item, has no key window to claim.
pub(crate) fn claim_for_element(
    element: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyboardClaim, AdapterError> {
    match element_attribute(element, "AXWindow", deadline)? {
        Some(window) => claim(&window, pid, deadline),
        None => Ok(KeyboardClaim::settled(KeyClaim::Refused(
            "element_has_no_window",
        ))),
    }
}

/// Claims the keyboard for the window that holds `element` and, for a web
/// runtime that focuses its content only then, keeps a background application
/// handling keys as the active one does while the claim lives.
pub(crate) fn claim_awake_for_element(
    element: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyboardClaim, AdapterError> {
    let claim = claim_for_element(element, pid, deadline)?;
    if claim.standing != KeyClaim::AlreadyKey {
        return Ok(claim);
    }
    Ok(wake(pid, deadline).unwrap_or(claim))
}

/// Makes a background application handle a shortcut as the active application
/// does, for as long as the claim lives. A background application takes plain
/// keys at once but leaves a key equivalent unhandled for seconds, or drops it.
/// Returns `None` when the application is in front, hidden or has no key window.
pub(crate) fn wake(pid: i32, deadline: Deadline) -> Option<KeyboardClaim> {
    if !hand_over_offered() {
        return None;
    }
    let app = crate::tree::element_for_pid(pid);
    if in_front(&app, pid, deadline) || flag(&app, "AXHidden", deadline) {
        return None;
    }
    let holder = key_window(&app, deadline).ok().flatten()?;
    let claimed = window_number(&holder, deadline)?;
    switch::focus(pid, &app, claimed, deadline).then(|| KeyboardClaim {
        standing: KeyClaim::AlreadyKey,
        hand_back: Some(HandBack {
            pid,
            app,
            claimed,
            previous: None,
            background: true,
        }),
    })
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
        "Keys posted to an application reach its key window. Retry in a moment if the person was \
         typing, or retry with --headed to bring this window forward.",
    )
}

#[derive(Debug, Clone, Copy)]
struct Standing {
    frontmost: bool,
    hidden: bool,
    minimized: bool,
    holder_is_transient: bool,
    hand_over_offered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Leave,
    HandOver,
    Refuse(&'static str),
}

/// What to do for a window that does not hold its application's keyboard.
fn route(standing: Standing) -> Route {
    if standing.hidden {
        Route::Refuse("application_hidden")
    } else if standing.minimized {
        Route::Refuse("window_minimized")
    } else if standing.holder_is_transient && standing.frontmost {
        Route::Refuse("transient_window_holds_keyboard")
    } else if standing.holder_is_transient {
        Route::Leave
    } else if standing.hand_over_offered {
        Route::HandOver
    } else {
        Route::Refuse("sibling_window_holds_keyboard")
    }
}

struct Parties<'a> {
    app: &'a AXElement,
    window: &'a AXElement,
    holder: Option<&'a AXElement>,
    pid: i32,
}

fn hand_over(
    parties: Parties<'_>,
    frontmost: bool,
    deadline: Deadline,
) -> Result<KeyboardClaim, AdapterError> {
    let Parties {
        app,
        window,
        holder,
        pid,
    } = parties;
    let refused = |cause| Ok(KeyboardClaim::settled(KeyClaim::Refused(cause)));
    let Some(to) = window_number(window, deadline) else {
        return refused("window_has_no_number");
    };
    let previous = holder
        .and_then(|holder| window_number(holder, deadline).map(|number| (number, holder.clone())));
    if frontmost && previous.is_none() {
        return refused("sibling_window_holds_keyboard");
    }
    if frontmost && !switch::person_went_idle(deadline) {
        return refused("person_is_typing");
    }
    let from = previous.as_ref().map(|(number, _)| *number);
    let step = Move {
        pid,
        app,
        from,
        to,
        window,
    };
    let handed = switch::switch(step, deadline, switch::CLAIM_ATTEMPTS);
    tracing::debug!(pid, ?from, to, handed, "window: keyboard handed over");
    let claim = KeyboardClaim {
        standing: KeyClaim::HandedOver,
        hand_back: Some(HandBack {
            pid,
            app: app.clone(),
            claimed: to,
            previous,
            background: !frontmost,
        }),
    };
    if handed {
        return Ok(claim);
    }
    drop(claim);
    refused("hand_over_not_observed")
}

/// The window server says which application is in front. The application's
/// own answer is the fallback, because one that holds a claimed keyboard
/// believes it is frontmost.
fn in_front(app: &AXElement, pid: i32, deadline: Deadline) -> bool {
    handoff::is_front_process(pid).unwrap_or_else(|| flag(app, "AXFrontmost", deadline))
}

fn hand_over_offered() -> bool {
    let enabled = std::env::var(HAND_OVER_SWITCH)
        .map(|value| !value.eq_ignore_ascii_case("off"))
        .unwrap_or(true);
    enabled && handoff::available()
}

/// A dialog, sheet or panel holds the keyboard for the window it belongs to,
/// so taking the keyboard from it would dismiss or bypass it.
fn is_transient(window: &AXElement, deadline: Deadline) -> bool {
    crate::tree::properties::attributes::copy_string_attr_result(window, "AXSubrole", deadline)
        .ok()
        .flatten()
        .is_some_and(|subrole| subrole != "AXStandardWindow")
}

fn window_number(window: &AXElement, deadline: Deadline) -> Option<u32> {
    let deadline = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    super::window_resolve::ax_window_id_with_deadline(window, deadline)
        .ok()
        .flatten()
        .and_then(|number| u32::try_from(number).ok())
}

fn flag(element: &AXElement, attribute: &str, deadline: Deadline) -> bool {
    crate::tree::copy_bool_attr(element, attribute, deadline) == Some(true)
}

#[cfg(test)]
#[path = "window_key_tests.rs"]
mod tests;
