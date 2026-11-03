//! Controls that live inside a web page (Safari, Chromium browsers, Electron apps).
//!
//! Web engines publish their DOM through an `AXWebArea`, and a few semantic writes behave
//! differently there than on AppKit controls: WebKit drops an `AXValue` or `AXSelectedText`
//! write on a text field that does not hold keyboard focus, and Chromium answers
//! `AXShowMenu` on a `<select>` with the page's context menu.

use agent_desktop_core::Deadline;

use super::AXElement;

const MAX_WEB_ANCESTRY: usize = 64;

/// True when the element sits inside an `AXWebArea`. Read failures count as "not web",
/// which keeps the native path unchanged.
pub(crate) fn is_inside_web_area(element: &AXElement, deadline: Deadline) -> bool {
    let Ok(instant) = super::locator_deadline::from_operation(deadline) else {
        return false;
    };
    let mut current = super::surface_read::element(element, "AXParent", instant)
        .ok()
        .flatten();
    for _ in 0..MAX_WEB_ANCESTRY {
        let Some(parent) = current else {
            return false;
        };
        let role = super::surface_read::string(&parent, "AXRole", instant)
            .ok()
            .flatten();
        if !continues_web_search(role.as_deref()) {
            return role.as_deref() == Some("AXWebArea");
        }
        current = super::surface_read::element(&parent, "AXParent", instant)
            .ok()
            .flatten();
    }
    false
}

/// Moves keyboard focus to a web text control inside its own application, without
/// activating the application. Returns whether the element is web content, so the
/// caller knows the web fallbacks apply even when the focus write was refused.
#[cfg(target_os = "macos")]
pub(crate) fn focus_web_text_control(element: &AXElement, deadline: Deadline) -> bool {
    if !is_inside_web_area(element, deadline) {
        return false;
    }
    let _ = crate::actions::ax_helpers::set_ax_bool_or_err(element, "AXFocused", true, deadline);
    true
}

/// Focuses a web text control for typing, inside its own application, as
/// `focus_web_text_control` does. Returns `None` for a control outside web content, and
/// otherwise whether the control already held focus with its caret moved off the start
/// or text selected, the one case where typed text should land at the caret.
#[cfg(target_os = "macos")]
pub(crate) fn focus_web_text_control_for_typing(
    element: &AXElement,
    deadline: Deadline,
) -> Option<bool> {
    if !is_inside_web_area(element, deadline) {
        return None;
    }
    let placed_caret = holds_placed_caret(
        super::copy_bool_attr(element, "AXFocused", deadline) == Some(true),
        super::properties::attributes::selected_text_range(element, deadline),
    );
    let _ = crate::actions::ax_helpers::set_ax_bool_or_err(element, "AXFocused", true, deadline);
    Some(placed_caret)
}

/// True when a web text control is being edited: it holds focus and its caret was moved
/// off the start, or text is selected. Typed text lands at that caret; otherwise it is
/// appended, because a focus write leaves a `contenteditable` caret at the start.
pub(crate) fn holds_placed_caret(focused: bool, selection: Option<std::ops::Range<usize>>) -> bool {
    focused && selection.is_some_and(|range| range.start > 0 || range.end > range.start)
}

/// Collapses the selection of a web text control to the end of its value. Chromium
/// puts the caret at the start of a `contenteditable` focused through accessibility,
/// where typing would prepend. Best effort: a refused write leaves the caret in place.
#[cfg(target_os = "macos")]
pub(crate) fn move_caret_to_end(element: &AXElement, deadline: Deadline) {
    use accessibility_sys::{AXValueCreate, kAXValueTypeCFRange};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFRange;

    let end = super::copy_value_typed(element, deadline)
        .unwrap_or_default()
        .encode_utf16()
        .count();
    let Ok(location) = isize::try_from(end) else {
        return;
    };
    let range = CFRange {
        location,
        length: 0,
    };
    let value = unsafe { AXValueCreate(kAXValueTypeCFRange, (&range as *const CFRange).cast()) };
    if value.is_null() {
        return;
    }
    let value = unsafe { CFType::wrap_under_create_rule(value as _) };
    let attribute = CFString::new("AXSelectedTextRange");
    let _ = super::properties::ax_ipc::set_attribute_value(
        element,
        attribute.as_concrete_TypeRef(),
        value.as_CFTypeRef(),
        deadline,
    );
}

/// `value` with `text` in place of the selection, as typing would leave it. `selection`
/// is in UTF-16 units, as `AXSelectedTextRange` reports it; without a usable selection the
/// text goes at the end.
pub(crate) fn splice_at_selection(
    value: &str,
    selection: Option<std::ops::Range<usize>>,
    text: &str,
) -> String {
    let units: Vec<u16> = value.encode_utf16().collect();
    let (start, end) = match selection {
        Some(range) if range.start <= range.end && range.end <= units.len() => {
            (range.start, range.end)
        }
        _ => (units.len(), units.len()),
    };
    let mut spliced = Vec::with_capacity(units.len() + text.len());
    spliced.extend_from_slice(&units[..start]);
    spliced.extend(text.encode_utf16());
    spliced.extend_from_slice(&units[end..]);
    String::from_utf16_lossy(&spliced)
}

fn continues_web_search(role: Option<&str>) -> bool {
    !matches!(
        role,
        Some("AXWebArea") | Some("AXWindow") | Some("AXApplication") | None
    )
}

#[cfg(test)]
mod tests {
    use super::{continues_web_search, holds_placed_caret, splice_at_selection};

    #[test]
    fn typed_web_text_keeps_only_a_caret_someone_placed() {
        assert!(
            holds_placed_caret(true, Some(3..3)),
            "caret moved off the start"
        );
        assert!(
            holds_placed_caret(true, Some(0..2)),
            "text selected from the start"
        );
        assert!(
            !holds_placed_caret(true, Some(0..0)),
            "a focus write leaves the caret at 0"
        );
        assert!(
            !holds_placed_caret(false, Some(5..5)),
            "an unfocused field is appended to"
        );
        assert!(!holds_placed_caret(true, None), "no selection reported");
    }

    #[test]
    fn typed_text_replaces_the_selection_or_lands_at_the_end() {
        assert_eq!(splice_at_selection("", Some(0..0), "hola"), "hola");
        assert_eq!(splice_at_selection("ab", None, "c"), "abc");
        assert_eq!(splice_at_selection("abcd", Some(1..3), "X"), "aXd");
        assert_eq!(splice_at_selection("ab", Some(1..9), "c"), "abc");
        assert_eq!(
            splice_at_selection("ñ😀z", Some(3..3), "!"),
            "ñ😀!z",
            "selection offsets count UTF-16 units: ñ is one, the emoji two"
        );
    }

    #[test]
    fn the_web_search_stops_at_the_web_area_or_the_window() {
        assert!(continues_web_search(Some("AXGroup")));
        assert!(continues_web_search(Some("AXScrollArea")));
        assert!(!continues_web_search(Some("AXWebArea")));
        assert!(!continues_web_search(Some("AXWindow")));
        assert!(!continues_web_search(Some("AXApplication")));
        assert!(!continues_web_search(None));
    }
}
