use agent_desktop_core::{AdapterError, Deadline};

use crate::system::window::window_key::{self, KeyboardClaim};
use crate::tree::AXElement;

/// Claims its application's keyboard for a web control before keys are posted
/// to the process. A runtime that focuses web content inside a background
/// application on its own keeps the application as it is, so a list that
/// closes on blur stays open between commands. Any other runtime also gets
/// the application awake, because it would otherwise refuse the focus or
/// bring the application forward to take it.
pub(crate) fn claim(
    element: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<KeyboardClaim, AdapterError> {
    if crate::tree::web_content::focuses_in_background(element, deadline) {
        window_key::claim_for_element(element, pid, deadline)
    } else {
        window_key::claim_awake_for_element(element, pid, deadline)
    }
}
