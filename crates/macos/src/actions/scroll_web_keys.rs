use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, KeyCombo, Rect,
};
use std::time::Duration;

use crate::tree::AXElement;

const MAX_WEB_AREA_DEPTH: usize = 64;
const SETTLE_STEP: Duration = Duration::from_millis(40);
const SETTLE_ATTEMPTS: u32 = 10;
const MOVE_EPSILON: f64 = 0.5;

/// Roles whose web controls consume arrow keys to change their own value or
/// selection, so the keys must go to the document instead of to them.
const ARROW_CONSUMING_ROLES: &[&str] = &[
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
    "AXPopUpButton",
    "AXRadioButton",
    "AXSlider",
    "AXIncrementor",
    "AXList",
    "AXTable",
    "AXOutline",
    "AXTabGroup",
    "AXMenu",
    "AXMenuBar",
    "AXMenuItem",
    "AXDateField",
    "AXColorWell",
];

/// Scrolls web content whose containers publish no scroll mechanism by sending
/// arrow keys to the owning process, one key per requested unit.
///
/// Process-targeted keys reach the application's key window even while the
/// application is in the background, so keys are sent only when the target
/// belongs to that window. A window behind a sibling window of the same
/// application gets no key, because the keys would land in the sibling.
/// Returns whether the target was seen to move.
pub(crate) fn scroll_with_keys(
    element: &AXElement,
    web_area: &AXElement,
    direction: &Direction,
    amount: u32,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let pid = crate::tree::properties::ax_ipc::pid(element, deadline).map_err(|_| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not read the process that owns the scroll target",
        )
    })?;
    ensure_key_window(element, pid, deadline)?;
    focus_scroll_context(element, web_area, deadline)?;
    let before = bounds(element, deadline).ok().flatten();
    let combo = KeyCombo {
        key: arrow_key(direction).into(),
        modifiers: Vec::new(),
    };
    let mut delivery = crate::actions::DeliveryTracker::default();
    for _ in 0..amount {
        crate::input::keyboard::synthesize_key(&combo, Some(pid), deadline)
            .map_err(|error| delivery.annotate(error))?;
        delivery.mark_delivered();
    }
    Ok(settled_move(element, before, direction, deadline))
}

/// The nearest web document that holds `element`, the element itself included.
pub(crate) fn enclosing_web_area(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut current = Some(element.clone());
    for _ in 0..MAX_WEB_AREA_DEPTH {
        let Some(candidate) = current else {
            return Ok(None);
        };
        if crate::actions::scroll_read::string(&candidate, "AXRole", deadline)?.as_deref()
            == Some("AXWebArea")
        {
            return Ok(Some(candidate));
        }
        current = crate::actions::scroll_read::element(&candidate, "AXParent", deadline)?;
    }
    Ok(None)
}

fn ensure_key_window(
    element: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let window = crate::actions::scroll_read::element(element, "AXWindow", deadline)?;
    let app = crate::tree::element_for_pid(pid);
    let key_window = match crate::actions::scroll_read::element(&app, "AXFocusedWindow", deadline)?
    {
        Some(window) => Some(window),
        None => crate::actions::scroll_read::element(&app, "AXMainWindow", deadline)?,
    };
    let same = window
        .as_ref()
        .zip(key_window.as_ref())
        .is_some_and(|(window, key_window)| {
            crate::tree::properties::capabilities::same_element(window, key_window)
        });
    if same {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "This web content scrolls only by keys, and keys would reach another window of the same application",
    )
    .with_details(serde_json::json!({
        "kind": "web_scroll_window_not_key",
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
    .with_suggestion(
        "Run 'scroll-to' on the element you want to see: it scrolls a background window without keys. \
         Or retry with --headed to bring this window forward.",
    ))
}

fn focus_scroll_context(
    element: &AXElement,
    web_area: &AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let role = crate::actions::scroll_read::string(element, "AXRole", deadline)?;
    let on_screen = bounds(element, deadline)
        .ok()
        .flatten()
        .zip(bounds(web_area, deadline).ok().flatten())
        .is_some_and(|(element, viewport)| overlaps(&element, &viewport));
    let target = if on_screen && !role.as_deref().is_some_and(consumes_arrow_keys) {
        element
    } else {
        web_area
    };
    if focus(target, deadline) {
        return Ok(());
    }
    if !crate::tree::properties::capabilities::same_element(target, web_area) {
        focus(web_area, deadline);
    }
    Ok(())
}

fn focus(element: &AXElement, deadline: Deadline) -> bool {
    matches!(
        crate::actions::ax_helpers::set_ax_bool_or_err(element, "AXFocused", true, deadline),
        Ok(true)
    )
}

fn bounds(element: &AXElement, deadline: Deadline) -> Result<Option<Rect>, AdapterError> {
    crate::tree::element_bounds::read_bounds_with_deadline(
        element,
        crate::tree::locator_deadline::from_operation(deadline)?,
    )
}

fn settled_move(
    element: &AXElement,
    before: Option<Rect>,
    direction: &Direction,
    deadline: Deadline,
) -> bool {
    let Some(before) = before else {
        return false;
    };
    for _ in 0..SETTLE_ATTEMPTS {
        let Ok(pause) = deadline.remaining_slice(SETTLE_STEP) else {
            return false;
        };
        std::thread::sleep(pause);
        if let Ok(Some(after)) = bounds(element, deadline)
            && moved_toward(&before, &after, direction)
        {
            return true;
        }
    }
    false
}

/// Focusing an element outside the visible document scrolls the document to
/// it first, so only an element that overlaps the viewport takes the focus.
fn overlaps(element: &Rect, viewport: &Rect) -> bool {
    element.x < viewport.x + viewport.width
        && element.x + element.width > viewport.x
        && element.y < viewport.y + viewport.height
        && element.y + element.height > viewport.y
}

fn moved_toward(before: &Rect, after: &Rect, direction: &Direction) -> bool {
    match direction {
        Direction::Down => after.y < before.y - MOVE_EPSILON,
        Direction::Up => after.y > before.y + MOVE_EPSILON,
        Direction::Right => after.x < before.x - MOVE_EPSILON,
        Direction::Left => after.x > before.x + MOVE_EPSILON,
    }
}

fn arrow_key(direction: &Direction) -> &'static str {
    match direction {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::Right => "right",
        Direction::Left => "left",
    }
}

fn consumes_arrow_keys(role: &str) -> bool {
    ARROW_CONSUMING_ROLES.contains(&role)
}

#[cfg(test)]
#[path = "scroll_web_keys_tests.rs"]
mod tests;
