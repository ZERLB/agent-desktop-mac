use super::{KeyClaim, KeyboardClaim, Route, Standing, refusal, route};
use agent_desktop_core::{DeliverySemantics, ErrorCode};

fn sibling_holds_keyboard() -> Standing {
    Standing {
        frontmost: false,
        hidden: false,
        minimized: false,
        holder_is_transient: false,
        hand_over_offered: true,
    }
}

#[test]
fn a_sibling_window_hands_the_keyboard_over_in_the_background_and_in_front() {
    assert_eq!(route(sibling_holds_keyboard()), Route::HandOver);
    assert_eq!(
        route(Standing {
            frontmost: true,
            ..sibling_holds_keyboard()
        }),
        Route::HandOver
    );
}

#[test]
fn without_the_hand_over_a_sibling_keeps_the_keyboard() {
    for frontmost in [false, true] {
        assert_eq!(
            route(Standing {
                frontmost,
                hand_over_offered: false,
                ..sibling_holds_keyboard()
            }),
            Route::Refuse("sibling_window_holds_keyboard")
        );
    }
}

#[test]
fn a_hidden_application_or_a_minimized_window_is_never_switched() {
    assert_eq!(
        route(Standing {
            hidden: true,
            ..sibling_holds_keyboard()
        }),
        Route::Refuse("application_hidden")
    );
    assert_eq!(
        route(Standing {
            minimized: true,
            ..sibling_holds_keyboard()
        }),
        Route::Refuse("window_minimized")
    );
}

#[test]
fn a_dialog_keeps_the_keyboard_it_holds_for_its_window() {
    let transient = Standing {
        holder_is_transient: true,
        ..sibling_holds_keyboard()
    };
    assert_eq!(route(transient), Route::Leave);
    assert_eq!(
        route(Standing {
            frontmost: true,
            ..transient
        }),
        Route::Refuse("transient_window_holds_keyboard")
    );
}

#[test]
fn only_a_handed_over_claim_reports_the_hand_over() {
    assert!(KeyboardClaim::settled(KeyClaim::HandedOver).handed_over());
    assert!(!KeyboardClaim::settled(KeyClaim::AlreadyKey).handed_over());
    assert_eq!(KeyboardClaim::settled(KeyClaim::AlreadyKey).refusal(), None);
    assert_eq!(
        KeyboardClaim::settled(KeyClaim::Refused("person_is_typing")).refusal(),
        Some("person_is_typing")
    );
}

#[test]
fn the_refusal_says_no_key_was_sent_and_names_the_window() {
    let error = refusal("person_is_typing", Some("w-7"));

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "window_not_key");
    assert_eq!(details["check"], "window_active");
    assert_eq!(details["cause"], "person_is_typing");
    assert_eq!(details["window_id"], "w-7");
    assert_eq!(details["physical_delivery_started"], false);
}
