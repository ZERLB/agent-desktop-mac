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
    covering_window(number, bounds).map(|front| front.is_some())
}

/// The frame of the first on-screen window in front of window `number` that
/// contains all of `bounds`. The outer `None` means the window server gave no
/// usable list.
pub(crate) fn covering_window(number: i64, bounds: &Rect) -> Option<Option<Rect>> {
    use core_graphics::window::kCGWindowListOptionOnScreenAboveWindow;

    let relative_to = u32::try_from(number).ok()?;
    let above = on_screen(
        kCGWindowListOptionOnScreenAboveWindow,
        Some(relative_to),
        WindowRecordScope::Every,
    )?;
    Some(
        above
            .into_iter()
            .find(|window| window.visible && covers(&window.bounds, bounds))
            .map(|window| window.bounds),
    )
}

/// The window of process `pid` that is in front of its other windows on
/// screen, or `None` when it shows none or the window server gave no list.
pub(crate) fn front_window_of(pid: i32) -> Option<u32> {
    use core_graphics::window::kCGWindowListOptionOnScreenOnly;

    on_screen(
        kCGWindowListOptionOnScreenOnly,
        None,
        WindowRecordScope::Pid(pid),
    )?
    .into_iter()
    .find(|window| window.visible)
    .and_then(|window| u32::try_from(window.window_number).ok())
}

/// On-screen windows from front to back.
fn on_screen(
    option: core_graphics::window::CGWindowListOption,
    relative_to: Option<u32>,
    scope: WindowRecordScope<'_>,
) -> Option<Vec<super::cg_window::WindowRecord>> {
    use crate::tree::properties::cf_type::borrowed_cf_dictionary;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::kCGWindowListExcludeDesktopElements;

    let options = option | kCGWindowListExcludeDesktopElements;
    let array = CGDisplay::window_list_info(options, relative_to)?;
    let dictionaries = array
        .get_all_values()
        .into_iter()
        .map(|raw| borrowed_cf_dictionary(raw as core_foundation::base::CFTypeRef))
        .collect::<Option<Vec<_>>>()?;
    records_from_dictionaries(dictionaries, scope).ok()
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
