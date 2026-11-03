use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};

use crate::tree::AXElement;

/// Types `text` into a text control. Headless, the text is inserted at the selection
/// through `AXSelectedText`. A web field is focused inside its own application and, once
/// it holds that application's keyboard focus, receives the text as key events posted to
/// its process, because rich web editors and script-driven fields ignore semantic writes.
/// Typed web text is appended unless the field was already focused with its caret placed
/// or text selected, since a focus write leaves a `contenteditable` caret at the start.
/// Two controls need the text spliced in through `AXValue` instead: a web field that does
/// not take focus, which Safari and Chromium accept the insertion on and then drop, and a
/// single-line native field such as a search field, which drops an `AXSelectedText` write
/// until it holds keyboard focus. An unfocused single-line field has no caret, so its text
/// is spliced at once; a focused one keeps the caret insertion. Multi-line text is never
/// rewritten through `AXValue`, which would drop its formatting.
pub(crate) fn execute_type(
    element: &AXElement,
    text: &str,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionStep, AdapterError> {
    let Some(role) = text_target_role(element, deadline)? else {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Type requires a text field, secure text field, or combo box",
        ));
    };
    if policy.is_headed() {
        crate::actions::physical_keyboard::type_text(element, text, policy, deadline)?;
        return Ok(ActionStep::succeeded("PIDTargetedUnicodeText")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false));
    }
    let web_caret = crate::tree::web_content::focus_web_text_control_for_typing(element, deadline);
    let web = web_caret.is_some();
    let append = web_caret == Some(false);
    if web
        && crate::actions::physical_keyboard::type_text_into_focused_web_control(
            element,
            text,
            append,
            !is_single_line(&role),
            deadline,
        )?
    {
        return Ok(ActionStep::succeeded("PIDTargetedUnicodeText")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false));
    }
    if append {
        crate::tree::web_content::move_caret_to_end(element, deadline);
    }
    let (before, selection) = value_and_selection(element, deadline);
    let unfocused_single_line = !web
        && is_single_line(&role)
        && crate::tree::copy_bool_attr(element, "AXFocused", deadline) != Some(true);
    if !unfocused_single_line {
        match insert_selected_text(element, text, deadline) {
            Ok(())
                if web
                    && crate::tree::copy_value_typed(element, deadline).unwrap_or_default()
                        == before => {}
            Ok(()) => return Ok(semantic_step("AXSelectedText")),
            Err(error)
                if error.disposition == DeliverySemantics::not_delivered()
                    && is_single_line(&role) => {}
            Err(error) => return Err(error),
        }
    }
    let caret = if unfocused_single_line {
        None
    } else {
        selection
    };
    let spliced = crate::tree::web_content::splice_at_selection(&before, caret, text);
    if let Err(error) =
        crate::actions::ax_helpers::set_ax_value_coerced(element, &spliced, deadline)
        && crate::tree::copy_value_typed(element, deadline).as_deref() != Some(&spliced)
    {
        return Err(error);
    }
    Ok(semantic_step("AXValue"))
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn is_single_line(role: &str) -> bool {
    matches!(role, "AXTextField" | "AXSecureTextField" | "AXComboBox")
}

/// Where typed text lands, in UTF-16 units. A single-line native field without keyboard
/// focus has no caret, and macOS reports its selection at 0 even though typing appends,
/// so its caret is the end of the value. A web field gets its text at the end too unless
/// it already holds focus with its caret placed or text selected, as `execute_type`
/// does. Any other control reports its own selection.
pub(crate) fn typing_caret(
    element: &AXElement,
    deadline: Deadline,
) -> Option<std::ops::Range<usize>> {
    let role =
        crate::tree::properties::attributes::copy_string_attr_result(element, "AXRole", deadline)
            .ok()
            .flatten();
    let focused = crate::tree::copy_bool_attr(element, "AXFocused", deadline) == Some(true);
    let selection = crate::tree::properties::attributes::selected_text_range(element, deadline);
    let appends = if crate::tree::web_content::is_inside_web_area(element, deadline) {
        !crate::tree::web_content::holds_placed_caret(focused, selection.clone())
    } else {
        role.as_deref().is_some_and(is_single_line) && !focused
    };
    if appends {
        let end = crate::tree::copy_value_typed(element, deadline)
            .unwrap_or_default()
            .encode_utf16()
            .count();
        return Some(end..end);
    }
    selection
}

fn value_and_selection(
    element: &AXElement,
    deadline: Deadline,
) -> (String, Option<std::ops::Range<usize>>) {
    (
        crate::tree::copy_value_typed(element, deadline).unwrap_or_default(),
        crate::tree::properties::attributes::selected_text_range(element, deadline),
    )
}

fn insert_selected_text(
    element: &AXElement,
    text: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    prepare(element, deadline)?;
    write_selected_text(text, |attribute, value| {
        crate::actions::ax_helpers::set_ax_string_or_err(element, attribute, value, deadline)
    })?;
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_details(serde_json::json!({ "operation": "AXSelectedText" }))
            .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(())
}

fn write_selected_text(
    text: &str,
    write: impl FnOnce(&str, &str) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    write("AXSelectedText", text)
}

fn text_target_role(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<String>, AdapterError> {
    prepare(element, deadline)?;
    let result =
        crate::tree::properties::attributes::copy_string_attr_result(element, "AXRole", deadline);
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let role = result.map_err(|error| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not read keyboard target role",
        )
        .with_details(serde_json::json!({ "ax_error": error }))
    })?;
    Ok(role.filter(|role| {
        matches!(
            role.as_str(),
            "AXTextField" | "AXTextArea" | "AXSecureTextField" | "AXComboBox"
        )
    }))
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn execute_type(
    _element: &crate::tree::AXElement,
    _text: &str,
    _policy: agent_desktop_core::InteractionPolicy,
    _deadline: Deadline,
) -> Result<agent_desktop_core::ActionStep, AdapterError> {
    Err(AdapterError::not_supported("type_text"))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    #[test]
    fn type_writes_the_current_selection_instead_of_the_whole_value() {
        let observed = RefCell::new(None);
        super::write_selected_text("inserted", |attribute, value| {
            observed.replace(Some((attribute.to_owned(), value.to_owned())));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            observed.into_inner(),
            Some(("AXSelectedText".into(), "inserted".into()))
        );
    }
}
