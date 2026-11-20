//! Keeps the person's application in front of a launch that was asked to
//! stay behind it.
//!
//! The launch request tells the system not to activate the application, and
//! an application can still bring itself forward while it starts. One that
//! does is put back behind the application the person was in, for as long as
//! the launch waits for its window.

use agent_desktop_core::{AdapterError, Deadline, system::launch_options::LaunchOptions};

use crate::system::window::app_front_guard::{self, AppFrontWatch};

/// Opens the application and returns it with the watch that keeps it behind
/// the person until the watch is dropped.
pub(crate) fn open(
    id: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<((i32, String), Option<AppFrontWatch>), AdapterError> {
    let person = (!options.activate)
        .then(app_front_guard::remember)
        .flatten();
    let launched = crate::system::launch_workspace::open(id, options, deadline)?;
    let front = person
        .filter(|person| !person.person_is_in(launched.0))
        .map(|person| person.watch_beside(launched.0));
    Ok((launched, front))
}
