//! Which running application an `--app` value names.
//!
//! On a Mac whose system language is not English, the localized name of a system app
//! differs from the name an agent usually knows ("Calculadora" for Calculator, "Notas"
//! for Notes). The bundle's file name (`Calculator.app`) stays the same in every
//! language, so it is accepted alongside the localized name and the bundle identifier.

/// True when `identifier` is the app's localized name, its bundle identifier, or the
/// file name of its bundle without the `.app` extension.
pub(super) fn identifies(
    name: &str,
    bundle_id: Option<&str>,
    bundle_name: Option<&str>,
    identifier: &str,
) -> bool {
    agent_desktop_core::app_name_matches(name, identifier)
        || bundle_id.is_some_and(|bundle_id| bundle_id.eq_ignore_ascii_case(identifier))
        || bundle_name.is_some_and(|bundle_name| {
            agent_desktop_core::app_name_matches(bundle_name, identifier)
        })
}

#[cfg(test)]
mod tests {
    use super::identifies;

    #[test]
    fn a_localized_app_answers_to_its_bundle_file_name() {
        let calculator = (
            "Calculadora",
            Some("com.apple.calculator"),
            Some("Calculator"),
        );
        for name in [
            "Calculadora",
            "calculator",
            "Calculator",
            "COM.APPLE.CALCULATOR",
        ] {
            assert!(
                identifies(calculator.0, calculator.1, calculator.2, name),
                "{name} should name the calculator"
            );
        }
        assert!(!identifies(
            calculator.0,
            calculator.1,
            calculator.2,
            "Calc"
        ));
        assert!(!identifies(calculator.0, calculator.1, None, "Calculator"));
    }
}
