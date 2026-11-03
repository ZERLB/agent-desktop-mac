//! Close control of a single Notification Center entry.
//!
//! Recent macOS releases publish a notification's close control as an accessibility
//! custom action (`Name:Close\nTarget:0x0\nSelector:(null)`) instead of a button, and
//! the label follows the system language ("Cerrar", "Fermer", "Schließen"). Labels match
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

/// The full custom-action string that closes this entry, ready for AXUIElementPerformAction.
#[cfg(target_os = "macos")]
pub(crate) fn custom_close_action(
    element: &crate::tree::AXElement,
    deadline: agent_desktop_core::Deadline,
) -> Option<String> {
    let mut usage = crate::tree::observation_usage::ObservationUsage::with_defaults();
    let read = crate::tree::properties::capabilities::copy_action_names_with_status(
        element,
        std::time::Instant::now() + deadline.remaining(),
        &mut usage,
    );
    read.value?
        .into_iter()
        .find(|action| custom_action_label(action).is_some_and(is_close_label))
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
