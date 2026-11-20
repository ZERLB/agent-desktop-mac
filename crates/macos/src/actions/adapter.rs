use agent_desktop_core::{
    Action, ActionOps, ActionResult, ActionStep, AdapterError, InteractionLease, NativeHandle,
    ObservationOps, StepMechanism, actions::action_request::ActionRequest,
};

use crate::adapter::{MacOSAdapter, ax_element};

impl ActionOps for MacOSAdapter {
    fn headed_requirement_for_ref(
        &self,
        entry: &agent_desktop_core::RefEntry,
        request: &ActionRequest,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::HeadedRequirement, AdapterError> {
        crate::actions::native_menu::headed_requirement(request, || {
            let handle = self.resolve_element_strict(entry, deadline)?;
            crate::tree::native_menu::is_native_menu(ax_element(&handle)?, deadline)
        })
    }

    fn headed_requirement(
        &self,
        handle: &NativeHandle,
        request: &ActionRequest,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::HeadedRequirement, AdapterError> {
        crate::actions::native_menu::headed_requirement(request, || {
            crate::tree::native_menu::is_native_menu(ax_element(handle)?, deadline)
        })
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if handle.is_null() {
            return execute_global_action(request, lease);
        }
        execute_action_impl(handle, request, lease)
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::actions::scroll_into_view::scroll_into_view_impl(
            ax_element(handle)?,
            lease.deadline(),
        )
    }
}

fn execute_action_impl(
    handle: &NativeHandle,
    request: ActionRequest,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let element = ax_element(handle)?;
    let front = crate::actions::person_front::watch(element, &request, lease.deadline());
    let result = crate::actions::perform_action(element, &request, lease.deadline());
    crate::actions::person_front::keep(front, &request.action, result)
}

fn execute_global_action(
    request: ActionRequest,
    lease: &InteractionLease,
) -> Result<ActionResult, AdapterError> {
    let Action::PressKey(combo) = request.action else {
        return Err(AdapterError::not_supported("global element action"));
    };
    crate::input::keyboard::synthesize_key(&combo, None, lease.deadline())?;
    Ok(
        ActionResult::delivered_unverified("press_key").with_steps(vec![
            ActionStep::succeeded("CGEventPost")
                .with_mechanism(StepMechanism::PhysicalSynthetic)
                .with_verified(false),
        ]),
    )
}
