//! Whether one window in front covers a window completely.
//!
//! A web runtime stops publishing the content of a page whose window shows no
//! pixel on screen, so a window behind a maximized one reads as nearly empty.
//! Only a single covering window is detected: several windows that cover it
//! together are not.

use agent_desktop_core::Rect;

use super::cg_window::{WindowRecordScope, records_from_dictionaries};

/// Whether an on-screen window in front of window `number` contains all of
/// `bounds`. `None` when the window server gives no usable list.
pub(crate) fn covered_by_one_window(number: i64, bounds: &Rect) -> Option<bool> {
    use crate::tree::properties::cf_type::borrowed_cf_dictionary;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenAboveWindow,
    };

    let relative_to = u32::try_from(number).ok()?;
    let options = kCGWindowListOptionOnScreenAboveWindow | kCGWindowListExcludeDesktopElements;
    let array = CGDisplay::window_list_info(options, Some(relative_to))?;
    let dictionaries = array
        .get_all_values()
        .into_iter()
        .map(|raw| borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef))
        .collect::<Option<Vec<_>>>()?;
    let above = records_from_dictionaries(dictionaries, WindowRecordScope::Every).ok()?;
    Some(
        above
            .iter()
            .any(|window| window.visible && covers(&window.bounds, bounds)),
    )
}

fn covers(front: &Rect, back: &Rect) -> bool {
    back.width > 0.0
        && back.height > 0.0
        && front.x <= back.x
        && front.y <= back.y
        && front.x + front.width >= back.x + back.width
        && front.y + front.height >= back.y + back.height
}

#[cfg(test)]
#[path = "cg_window_cover_tests.rs"]
mod tests;
