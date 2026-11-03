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
    use super::{continues_web_search, splice_at_selection};

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
