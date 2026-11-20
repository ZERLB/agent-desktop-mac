use super::covered_note_for;

#[test]
fn a_covered_window_carries_the_note_and_the_way_out() {
    let note = covered_note_for(Some(true)).expect("a note");

    assert_eq!(note["covered"], true);
    assert!(
        note["note"]
            .as_str()
            .is_some_and(|text| text.contains("move-window"))
    );
}

#[test]
fn a_window_that_shows_or_an_unknown_state_carries_no_note() {
    assert_eq!(covered_note_for(Some(false)), None);
    assert_eq!(covered_note_for(None), None);
}
