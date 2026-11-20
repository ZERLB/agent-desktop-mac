use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionPolicy, KeyCombo, ProcessIdentity, StepMechanism, WindowInfo,
};
use std::time::{Duration, Instant};

use crate::system::window::person_front::{self, Patience, PersonFront};
use crate::tree::AXElement;

const SHORTCUT_PATIENCE: Patience = Patience {
    window_looks: 12,
    app_looks: 30,
};

#[path = "key_dispatch_menu.rs"]
mod menu;

pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let front = person_front(&process, combo, policy, deadline);
    let result = {
        let _awake = wake_for_shortcut(&process, combo, policy, deadline);
        deliver(process, combo, policy, deadline)
    };
    keep_person_in_front(front, result)
}

/// A headless shortcut can open a new window, which comes up in front of the
/// window the person is using in that application, or brings a background
/// application forward with it.
fn person_front(
    process: &ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Option<PersonFront> {
    if policy.allow_focus_steal || !is_shortcut(combo) {
        return None;
    }
    let pid = crate::system::process::process_identity::require_core(process)
        .ok()?
        .pid();
    Some(person_front::watch(pid, SHORTCUT_PATIENCE, deadline))
}

fn keep_person_in_front(
    front: Option<PersonFront>,
    result: Result<ActionResult, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    match front {
        Some(front) => front.keep(SHORTCUT_PATIENCE, result),
        None => result,
    }
}

fn deliver(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    press_for_app(process, combo, policy, deadline)
        .map_err(|error| crate::actions::DeliveryTracker::default().annotate(error))
}

/// A background application leaves a shortcut unhandled for seconds, or drops
/// it, though it takes plain keys at once, so a headless shortcut is sent
/// while the application handles keys as the active one does.
fn wake_for_shortcut(
    process: &ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Option<crate::system::window::window_key::KeyboardClaim> {
    if policy.allow_focus_steal || !is_shortcut(combo) {
        return None;
    }
    let pid = crate::system::process::process_identity::require_core(process)
        .ok()?
        .pid();
    crate::system::window::window_key::wake(pid, deadline)
}

/// A background application whose web runtime focuses nothing until it is
/// awake reports no focused element, so a headless key that finds none wakes
/// the application and looks again before it gives up.
fn focused_or_awake(
    app: &AXElement,
    pid: i32,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Option<crate::system::window::window_key::KeyboardClaim>, AdapterError> {
    let Err(error) = require_focused_element(app, policy, deadline) else {
        return Ok(None);
    };
    if policy.allow_focus_steal {
        return Err(error);
    }
    let Some(awake) = crate::system::window::window_key::wake(pid, deadline) else {
        return Err(error);
    };
    require_focused_element(app, policy, deadline)?;
    Ok(Some(awake))
}

fn is_shortcut(combo: &KeyCombo) -> bool {
    combo
        .modifiers
        .iter()
        .any(|modifier| !matches!(modifier, agent_desktop_core::Modifier::Shift))
}

/// Sends `combo` to the process that owns `window` while that window holds the
/// application's keyboard. Keys posted to a process reach its key window, so a
/// window behind a sibling is handed the keyboard for the key and the sibling
/// gets it back afterwards.
pub(crate) fn press_for_window_impl(
    window: &WindowInfo,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            AdapterError::stale_ref_because("Window has no process-instance identity")
                .with_disposition(DeliverySemantics::not_delivered())
        })?;
    let process = ProcessIdentity::new(window.pid, instance);
    crate::system::process::process_identity::require_core(&process)?;
    let element = crate::system::window::window_resolve::window_element_for_info(window, deadline)?;
    let front = person_front(&process, combo, policy, deadline);
    let target = TargetWindow {
        element: &element,
        id: &window.id,
    };
    let result = press_in_claimed_window(target, process, combo, policy, deadline);
    keep_person_in_front(front, result)
}

struct TargetWindow<'a> {
    element: &'a AXElement,
    id: &'a str,
}

fn press_in_claimed_window(
    target: TargetWindow<'_>,
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let pid = crate::system::process::process_identity::require_core(&process)?.pid();
    let claim = crate::system::window::window_key::claim(target.element, pid, deadline)?;
    if let Some(cause) = claim.refusal() {
        return Err(crate::system::window::window_key::refusal(
            cause,
            Some(target.id),
        ));
    }
    let _awake = (!claim.handed_over())
        .then(|| wake_for_shortcut(&process, combo, policy, deadline))
        .flatten();
    let mut result = deliver(process, combo, policy, deadline)?;
    if claim.handed_over() {
        result.steps.insert(0, semantic_step("KeyWindowHandOver"));
    }
    Ok(result)
}

fn press_for_app(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    tracing::debug!(pid = process.pid.get(), "system: press key for app");
    let identity = crate::system::process::process_identity::require_core(&process)?;
    let pid = identity.pid();
    let app = crate::tree::element_for_pid(pid);

    if uses_menu_shortcut(combo, policy) {
        match menu::try_menu_bar_shortcut(&app, combo, deadline) {
            Ok(true) => {
                return post_delivery_process_result(&process, semantic_step("AXPress menu item"));
            }
            Ok(false) => {}
            Err(error) if menu::is_bounded_menu_incomplete(&error) => {
                tracing::debug!("bounded menu lookup fell back to exact PID-targeted key delivery");
            }
            Err(error) => return Err(error),
        }
    }
    if let Some(action) = simple_key_action(combo)
        && try_simple_key_action(&app, action, deadline)?
    {
        return post_delivery_process_result(&process, semantic_step(action));
    }

    crate::input::keyboard::keyboard_map::key_name_to_code(&combo.key)?;
    if policy.allow_focus_steal {
        crate::system::process::process_identity::require_core(&process)?;
        crate::system::focus::verify_app_focused(pid, deadline)?;
    }
    crate::system::process::process_identity::require_core(&process)?;
    let _awake = focused_or_awake(&app, pid, policy, deadline)?;
    crate::system::process::process_identity::require_core(&process)?;
    crate::input::keyboard::synthesize_key(combo, Some(pid), deadline)?;
    post_delivery_process_result(
        &process,
        ActionStep::succeeded("CGEventPostToPid")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false),
    )
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn post_delivery_process_result(
    process: &ProcessIdentity,
    step: ActionStep,
) -> Result<ActionResult, AdapterError> {
    crate::system::process::process_identity::require_core(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    Ok(ActionResult::delivered_unverified("press_key").with_steps(vec![step]))
}

fn try_simple_key_action(
    app: &AXElement,
    action: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(focused) = focused_element(app, deadline)? else {
        return Ok(false);
    };
    if crate::actions::ax_helpers::advertises_action(&focused, action, deadline) != Some(true) {
        return Ok(false);
    }
    crate::actions::ax_helpers::try_ax_action_or_err(&focused, action, deadline)
}

fn uses_menu_shortcut(combo: &KeyCombo, policy: InteractionPolicy) -> bool {
    policy.allow_focus_steal && !combo.modifiers.is_empty()
}

fn simple_key_action(combo: &KeyCombo) -> Option<&'static str> {
    if !combo.modifiers.is_empty() {
        return None;
    }
    match combo.key.as_str() {
        "return" | "enter" => Some("AXConfirm"),
        "escape" | "esc" => Some("AXCancel"),
        _ => None,
    }
}

fn no_focused_element_error(policy: InteractionPolicy) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Application has no verified focused element for keyboard delivery",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_suggestion(if policy.allow_focus_steal {
        "Ensure the target field has keyboard focus before retrying, or run the menu command without keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref."
    } else {
        "Retry with --headed so the app can take focus, or run the menu command without \
         keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref."
    })
}

fn require_focused_element(
    app: &AXElement,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<AXElement, AdapterError> {
    let local_deadline = local_deadline(deadline, Duration::from_millis(500))?;
    loop {
        if let Some(focused) = focused_element(app, deadline)? {
            return Ok(focused);
        }
        deadline.ensure_remaining()?;
        if Instant::now() >= local_deadline {
            return Err(no_focused_element_error(policy));
        }
        let pause = deadline.remaining_slice(Duration::from_millis(5))?;
        std::thread::sleep(pause.min(Duration::from_millis(5)));
    }
}

fn focused_element(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    element_attribute(app, "AXFocusedUIElement", deadline)
}

fn element_attribute(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result =
        crate::tree::properties::attributes::copy_element_attr_result(element, attribute, deadline);
    deadline.ensure_remaining()?;
    result.map_err(|error| read_error(attribute, error))
}

fn local_deadline(deadline: Deadline, maximum: Duration) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining_slice(maximum)?;
    Instant::now()
        .checked_add(remaining)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Accessibility read failed for {attribute} during key dispatch"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn press_for_app_impl(
    _process: ProcessIdentity,
    _combo: &KeyCombo,
    _policy: InteractionPolicy,
    _deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_for_app"))
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
