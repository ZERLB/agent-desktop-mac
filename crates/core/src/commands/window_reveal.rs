//! Lets a completely covered web window show a strip of itself.
//!
//! A web runtime stops publishing a page whose window shows no pixel on
//! screen, so a browser window behind a maximized one reads as nearly empty.
//! A headless snapshot of such a window slides it to a screen edge the window
//! in front leaves free and reads it again. The window stays behind and takes
//! no focus.

use crate::{
    WindowOp, adapter::PlatformAdapter, context::CommandContext, snapshot::SnapshotResult,
};
use serde_json::{Value, json};
use std::time::Duration;

const SWITCH: &str = "AGENT_DESKTOP_UNCOVER";
const SETTLE: Duration = Duration::from_millis(500);

/// Moves the window that `result` read when it is completely covered and
/// hosts web content. Returns the window to read again and the note that says
/// where it went, or `None` when nothing was moved.
pub(crate) fn reveal(
    result: &SnapshotResult,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Option<(String, Value)> {
    if context.physical_input_policy().is_headed() || !enabled() {
        return None;
    }
    let crate::SnapshotOwner::Window(window) = &result.owner else {
        return None;
    };
    let from = window.bounds.as_ref()?;
    let to = adapter.peek_position(window)?;
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter).ok()?;
    let live =
        crate::commands::helpers::revalidate_window_for_mutation(adapter, window, &lease).ok()?;
    adapter
        .window_op(&live, WindowOp::Move { x: to.x, y: to.y }, &lease)
        .ok()?;
    drop(lease);
    std::thread::sleep(SETTLE);
    Some((window.id.clone(), moved_note(from.x, from.y, &to)))
}

fn moved_note(from_x: f64, from_y: f64, to: &crate::Point) -> Value {
    json!({
        "covered": true,
        "moved": true,
        "from": { "x": from_x, "y": from_y },
        "to": { "x": to.x, "y": to.y },
        "note": "Another window covered this window completely, and a web page publishes little or none of its content while covered. The window was moved so a strip of it shows beside the window in front, and read again. It stayed behind and took no focus. Set AGENT_DESKTOP_UNCOVER=off to leave windows where they are.",
    })
}

fn enabled() -> bool {
    std::env::var(SWITCH)
        .map(|value| !value.eq_ignore_ascii_case("off"))
        .unwrap_or(true)
}

#[cfg(test)]
#[path = "window_reveal_tests.rs"]
mod tests;
