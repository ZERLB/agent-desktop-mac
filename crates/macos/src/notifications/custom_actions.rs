//! Accessibility custom actions of a Notification Center entry.
//!
//! Recent macOS releases publish a notification's controls as accessibility custom actions
//! (`Name:Close\nTarget:0x0\nSelector:(null)`, `Name:Reply…`) instead of buttons, and the
//! labels follow the system language ("Cerrar", "Responder", "Archivar"). Close labels match
//! exactly, so stack-wide actions such as "Clear All" or "Borrar todo" never match.

/// Close labels by system language. Exact, case-insensitive matches only.
const CLOSE_LABELS: [&str; 16] = [
    "close",
    "clear",
    "dismiss",
    "cerrar",
    "fermer",
    "schließen",
    "fechar",
    "chiudi",
    "sluiten",
    "stäng",
    "zamknij",
    "kapat",
    "закрыть",
    "閉じる",
    "关闭",
    "關閉",
];

/// True when a button title or custom-action label names the entry's own close control.
pub(crate) fn is_close_label(label: &str) -> bool {
    let label = label.trim().to_lowercase();
    CLOSE_LABELS.iter().any(|close| label == *close)
}

/// The label of a custom action (`Name:<label>\nTarget:…`), or `None` for a standard
/// action such as `AXPress`.
pub(crate) fn custom_action_label(action: &str) -> Option<&str> {
    let rest = action.strip_prefix("Name:")?;
    Some(rest.split('\n').next().unwrap_or(rest).trim())
}

/// Every custom-action string the entry publishes, ready for AXUIElementPerformAction.
#[cfg(target_os = "macos")]
fn custom_actions(
    element: &crate::tree::AXElement,
    deadline: agent_desktop_core::Deadline,
) -> Vec<String> {
    let mut usage = crate::tree::observation_usage::ObservationUsage::with_defaults();
    crate::tree::properties::capabilities::copy_action_names_with_status(
        element,
        std::time::Instant::now() + deadline.remaining(),
        &mut usage,
    )
    .value
    .unwrap_or_default()
    .into_iter()
    .filter(|action| custom_action_label(action).is_some())
    .collect()
}

/// The custom action that closes this entry.
#[cfg(target_os = "macos")]
pub(crate) fn custom_close_action(
    element: &crate::tree::AXElement,
    deadline: agent_desktop_core::Deadline,
) -> Option<String> {
    custom_actions(element, deadline)
        .into_iter()
        .find(|action| custom_action_label(action).is_some_and(is_close_label))
}

/// Labels of the entry's custom actions an agent can press with notification-action,
/// such as "Responder" or "Archivar". Close actions are left to dismiss-notification.
#[cfg(target_os = "macos")]
pub(crate) fn pressable_labels(
    element: &crate::tree::AXElement,
    deadline: agent_desktop_core::Deadline,
) -> Vec<String> {
    custom_actions(element, deadline)
        .iter()
        .filter_map(|action| custom_action_label(action))
        .filter(|label| !label.is_empty() && !is_close_label(label))
        .map(str::to_owned)
        .collect()
}

/// Performs the custom action labelled `name` (case-insensitive) on notification `index`.
#[cfg(target_os = "macos")]
pub(crate) fn perform_named(
    element: &crate::tree::AXElement,
    name: &str,
    index: usize,
    deadline: agent_desktop_core::Deadline,
) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
    let wanted = name.trim().to_lowercase();
    let action = custom_actions(element, deadline)
        .into_iter()
        .find(|action| {
            custom_action_label(action).is_some_and(|label| label.to_lowercase() == wanted)
        });
    let performed = match &action {
        Some(action) => {
            crate::actions::ax_helpers::try_ax_action_or_err(element, action, deadline)?
        }
        None => false,
    };
    if !performed {
        return Err(agent_desktop_core::AdapterError::new(
            agent_desktop_core::ErrorCode::ActionFailed,
            format!("Action '{name}' not found on notification {index}"),
        ));
    }
    Ok(agent_desktop_core::ActionResult::delivered_unverified(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_action_labels_are_read_from_the_name_line() {
        assert_eq!(
            custom_action_label("Name:Cerrar\nTarget:0x0\nSelector:(null)"),
            Some("Cerrar")
        );
        assert_eq!(custom_action_label("Name:Close"), Some("Close"));
        assert_eq!(custom_action_label("AXPress"), None);
    }

    #[test]
    fn close_labels_match_every_language_but_never_clear_all() {
        for label in [
            "Close",
            "Cerrar",
            "Fermer",
            "Schließen",
            "閉じる",
            " clear ",
        ] {
            assert!(is_close_label(label), "{label} should close one entry");
        }
        for label in [
            "Clear All",
            "Borrar todo",
            "Mostrar detalles",
            "Responder",
            "",
        ] {
            assert!(!is_close_label(label), "{label} must not close the entry");
        }
    }
}
