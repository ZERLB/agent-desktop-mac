//! Keeps the person in front of a headless action on an element.
//!
//! Pressing a control can open a window, and writing the focus makes a
//! WebKit application activate itself. Every headless action is watched for
//! its application coming forward; the ones that press a control are also
//! given time for a window they open to appear.

use agent_desktop_core::{
    Action, ActionResult, AdapterError, Deadline, actions::action_request::ActionRequest,
};

use crate::system::window::person_front::{self, Patience, PersonFront};
use crate::tree::AXElement;

const PRESS: Patience = Patience {
    window_looks: 5,
    app_looks: 12,
};
const FOCUS: Patience = Patience {
    window_looks: 0,
    app_looks: 12,
};
const OTHER: Patience = Patience {
    window_looks: 0,
    app_looks: 1,
};

pub(crate) fn watch(
    element: &AXElement,
    request: &ActionRequest,
    deadline: Deadline,
) -> Option<PersonFront> {
    if request.policy.allow_focus_steal {
        return None;
    }
    let pid = crate::tree::properties::ax_ipc::pid(element, deadline).ok()?;
    Some(person_front::watch(
        pid,
        patience(&request.action),
        deadline,
    ))
}

pub(crate) fn keep(
    front: Option<PersonFront>,
    action: &Action,
    result: Result<ActionResult, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    match front {
        Some(front) => front.keep(patience(action), result),
        None => result,
    }
}

fn patience(action: &Action) -> Patience {
    match action {
        Action::Click
        | Action::DoubleClick
        | Action::TripleClick
        | Action::Select(_)
        | Action::Toggle
        | Action::Check
        | Action::Uncheck => PRESS,
        Action::SetFocus => FOCUS,
        _ => OTHER,
    }
}

#[cfg(test)]
#[path = "person_front_tests.rs"]
mod tests;
