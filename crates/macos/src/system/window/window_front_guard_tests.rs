use super::is_new_standard_window;

#[test]
fn a_standard_window_the_application_did_not_have_took_the_front() {
    assert!(is_new_standard_window(false, Some("AXStandardWindow")));
}

#[test]
fn a_window_that_was_already_open_is_the_persons_own_choice() {
    assert!(!is_new_standard_window(true, Some("AXStandardWindow")));
}

#[test]
fn a_dialog_or_sheet_stays_in_front_of_the_window_it_belongs_to() {
    for subrole in [
        Some("AXDialog"),
        Some("AXSheet"),
        Some("AXSystemDialog"),
        None,
    ] {
        assert!(!is_new_standard_window(false, subrole));
    }
}
