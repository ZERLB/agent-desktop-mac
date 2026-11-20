use super::moved_note;

#[test]
fn the_note_says_where_the_window_was_and_where_it_went() {
    let note = moved_note(44.0, 74.0, &crate::Point { x: 44.0, y: 141.0 });

    assert_eq!(note["covered"], true);
    assert_eq!(note["moved"], true);
    assert_eq!(note["from"]["y"], 74.0);
    assert_eq!(note["to"]["y"], 141.0);
    assert!(
        note["note"]
            .as_str()
            .is_some_and(|text| text.contains("AGENT_DESKTOP_UNCOVER"))
    );
}
