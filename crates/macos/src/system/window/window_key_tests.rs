use super::{KeyClaim, refusal, refusal_cause};
use agent_desktop_core::{DeliverySemantics, ErrorCode};

#[test]
fn a_background_visible_application_lets_its_key_window_change() {
    assert_eq!(refusal_cause(false, false, false), None);
}

#[test]
fn the_frontmost_application_keeps_the_key_window_the_person_is_in() {
    assert_eq!(
        refusal_cause(true, false, false),
        Some("sibling_window_holds_keyboard")
    );
    assert_eq!(
        refusal_cause(true, true, true),
        Some("sibling_window_holds_keyboard")
    );
}

#[test]
fn a_hidden_application_or_a_minimized_window_is_not_switched() {
    assert_eq!(
        refusal_cause(false, true, false),
        Some("application_hidden")
    );
    assert_eq!(refusal_cause(false, false, true), Some("window_minimized"));
}

#[test]
fn only_a_switched_window_reports_that_it_was_made_key() {
    assert!(KeyClaim::MadeKey.made_key());
    assert!(!KeyClaim::AlreadyKey.made_key());
    assert!(!KeyClaim::Refused("sibling_window_holds_keyboard").made_key());
}

#[test]
fn the_refusal_says_no_key_was_sent_and_names_the_window() {
    let error = refusal("sibling_window_holds_keyboard", Some("w-7"));

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "window_not_key");
    assert_eq!(details["check"], "window_active");
    assert_eq!(details["cause"], "sibling_window_holds_keyboard");
    assert_eq!(details["window_id"], "w-7");
    assert_eq!(details["physical_delivery_started"], false);
}
