use super::{
    FOCUS_GAINED, FOCUS_LOST, KIND_POINTER_DOWN, KIND_POINTER_UP, RECORD_LEN, focus_record,
    pointer_record,
};

const WINDOW: u32 = 0x0102_0304;

fn window_bytes(record: &[u8; RECORD_LEN]) -> [u8; 4] {
    [record[0x3c], record[0x3d], record[0x3e], record[0x3f]]
}

#[test]
fn every_record_states_its_length_and_names_the_window() {
    for record in [
        focus_record(WINDOW, FOCUS_LOST),
        focus_record(WINDOW, FOCUS_GAINED),
        pointer_record(WINDOW, KIND_POINTER_DOWN),
        pointer_record(WINDOW, KIND_POINTER_UP),
    ] {
        assert_eq!(usize::from(record[0x04]), RECORD_LEN);
        assert_eq!(window_bytes(&record), WINDOW.to_ne_bytes());
    }
}

#[test]
fn a_focus_record_says_whether_the_window_gained_or_lost_the_keyboard() {
    let lost = focus_record(WINDOW, FOCUS_LOST);
    let gained = focus_record(WINDOW, FOCUS_GAINED);

    assert_eq!((lost[0x08], lost[0x8a]), (0x0d, 0x02));
    assert_eq!((gained[0x08], gained[0x8a]), (0x0d, 0x01));
    assert!(lost[0x20..0x30].iter().all(|byte| *byte == 0));
}

#[test]
fn a_pointer_record_presses_the_window_at_no_point_inside_it() {
    let down = pointer_record(WINDOW, KIND_POINTER_DOWN);
    let up = pointer_record(WINDOW, KIND_POINTER_UP);

    assert_eq!((down[0x08], up[0x08]), (0x01, 0x02));
    for record in [down, up] {
        assert_eq!(record[0x3a], 0x10);
        assert!(record[0x20..0x30].iter().all(|byte| *byte == 0xff));
        assert_eq!(record[0x8a], 0);
    }
}
