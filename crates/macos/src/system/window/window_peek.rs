//! Where a completely covered window can show a strip of itself.
//!
//! A web runtime stops publishing a page whose window shows no pixel on
//! screen. Sliding the window until it reaches a screen edge that the window
//! in front leaves free keeps the rest of it behind and the page published.

use agent_desktop_core::{Deadline, Point, Rect, WindowInfo};

use crate::tree::AXElement;

const MIN_FREE_EDGE: f64 = 12.0;
const WEB_SEARCH_BUDGET_MS: u64 = 600;
const WEB_SEARCH_NODES: usize = 400;
const WEB_SEARCH_DEPTH: usize = 14;

/// The position that lets a strip of `window` show, when another window
/// covers it completely, it hosts web content and its screen has a free edge.
pub(crate) fn peek_position(window: &WindowInfo) -> Option<Point> {
    let number = window.id.strip_prefix("w-")?.parse().ok()?;
    let bounds = window.bounds.as_ref()?;
    let front = super::cg_window_cover::covering_window(number, bounds)??;
    let screen = screen_holding(bounds)?;
    let position = placement(bounds, &front, &screen)?;
    hosts_web_content(window).then_some(position)
}

/// Slides `back` to the edge of `screen` where `front` leaves the widest free
/// band, keeping the other coordinate. The top edge is the menu bar's and is
/// never used. `None` when no band is wide enough or the window would not fit.
fn placement(back: &Rect, front: &Rect, screen: &Rect) -> Option<Point> {
    let screen_right = screen.x + screen.width;
    let screen_bottom = screen.y + screen.height;
    let below = screen_bottom - (front.y + front.height);
    let right = screen_right - (front.x + front.width);
    let left = front.x - screen.x;
    let candidates = [
        (below, back.x, screen_bottom - back.height),
        (right, screen_right - back.width, back.y),
        (left, screen.x, back.y),
    ];
    candidates
        .into_iter()
        .filter(|(free, x, y)| *free >= MIN_FREE_EDGE && *x >= screen.x && *y >= screen.y)
        .max_by(|first, second| first.0.total_cmp(&second.0))
        .map(|(_, x, y)| Point { x, y })
}

fn screen_holding(bounds: &Rect) -> Option<Rect> {
    use core_graphics::display::CGDisplay;

    let center = (
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    );
    CGDisplay::active_displays()
        .ok()?
        .into_iter()
        .map(|display| CGDisplay::new(display).bounds())
        .map(|frame| Rect {
            x: frame.origin.x,
            y: frame.origin.y,
            width: frame.size.width,
            height: frame.size.height,
        })
        .find(|screen| {
            center.0 >= screen.x
                && center.0 < screen.x + screen.width
                && center.1 >= screen.y
                && center.1 < screen.y + screen.height
        })
}

/// Only a window with web content is worth moving: a native application keeps
/// publishing its content while covered. A covered page can publish no web
/// area at all, so the application's own make-up is asked first.
fn hosts_web_content(window: &WindowInfo) -> bool {
    let chromium = crate::system::process::process_identity::to_pid_t(window.pid)
        .is_ok_and(crate::system::renderer_kind::ships_renderer_helper);
    chromium || publishes_web_area(window)
}

fn publishes_web_area(window: &WindowInfo) -> bool {
    let Ok(deadline) = Deadline::detached_after(WEB_SEARCH_BUDGET_MS) else {
        return false;
    };
    let Ok(root) = super::window_resolve::window_element_for_info(window, deadline) else {
        return false;
    };
    let Ok(instant) = crate::tree::locator_deadline::from_operation(deadline) else {
        return false;
    };
    let mut level = vec![root];
    let mut seen = 0_usize;
    for _ in 0..WEB_SEARCH_DEPTH {
        let mut next: Vec<AXElement> = Vec::new();
        for element in &level {
            seen += 1;
            if seen > WEB_SEARCH_NODES {
                return false;
            }
            let role = crate::tree::surface_read::string(element, "AXRole", instant)
                .ok()
                .flatten();
            if role.as_deref() == Some("AXWebArea") {
                return true;
            }
            next.extend(
                crate::tree::surface_read::elements(element, "AXChildren", instant)
                    .unwrap_or_default(),
            );
        }
        if next.is_empty() {
            return false;
        }
        level = next;
    }
    false
}

#[cfg(test)]
#[path = "window_peek_tests.rs"]
mod tests;
