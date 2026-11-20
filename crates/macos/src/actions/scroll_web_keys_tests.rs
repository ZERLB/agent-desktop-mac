use super::{arrow_key, consumes_arrow_keys, intersection, moved_toward, overlaps};
use agent_desktop_core::{Direction, Rect};

fn rect(x: f64, y: f64) -> Rect {
    Rect {
        x,
        y,
        width: 100.0,
        height: 20.0,
    }
}

#[test]
fn content_moves_against_the_scroll_direction() {
    let before = rect(10.0, 300.0);
    assert!(moved_toward(&before, &rect(10.0, 180.0), &Direction::Down));
    assert!(!moved_toward(&before, &rect(10.0, 420.0), &Direction::Down));
    assert!(moved_toward(&before, &rect(10.0, 420.0), &Direction::Up));
    assert!(moved_toward(
        &before,
        &rect(-30.0, 300.0),
        &Direction::Right
    ));
    assert!(moved_toward(&before, &rect(50.0, 300.0), &Direction::Left));
}

#[test]
fn subpixel_jitter_is_not_movement() {
    let before = rect(10.0, 300.0);
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        assert!(!moved_toward(&before, &rect(10.2, 300.3), &direction));
    }
}

#[test]
fn each_direction_maps_to_its_arrow_key() {
    assert_eq!(arrow_key(&Direction::Down), "down");
    assert_eq!(arrow_key(&Direction::Up), "up");
    assert_eq!(arrow_key(&Direction::Left), "left");
    assert_eq!(arrow_key(&Direction::Right), "right");
}

#[test]
fn value_controls_hand_the_keys_to_the_document() {
    for role in [
        "AXTextField",
        "AXTextArea",
        "AXPopUpButton",
        "AXSlider",
        "AXList",
    ] {
        assert!(consumes_arrow_keys(role), "{role} consumes arrow keys");
    }
    for role in ["AXLink", "AXButton", "AXStaticText", "AXGroup", "AXWebArea"] {
        assert!(!consumes_arrow_keys(role), "{role} passes arrow keys on");
    }
}

#[test]
fn only_an_element_inside_the_viewport_counts_as_on_screen() {
    let viewport = Rect {
        x: 0.0,
        y: 100.0,
        width: 800.0,
        height: 600.0,
    };
    assert!(overlaps(&rect(10.0, 300.0), &viewport));
    assert!(overlaps(&rect(10.0, 690.0), &viewport));
    assert!(!overlaps(&rect(10.0, 700.0), &viewport));
    assert!(!overlaps(&rect(10.0, 60.0), &viewport));
    assert!(!overlaps(&rect(900.0, 300.0), &viewport));
}

#[test]
fn a_whole_document_frame_is_clipped_to_the_container_that_shows_it() {
    let document = Rect {
        x: 0.0,
        y: -1500.0,
        width: 1200.0,
        height: 4000.0,
    };
    let container = Rect {
        x: 0.0,
        y: 90.0,
        width: 1200.0,
        height: 800.0,
    };

    let visible = intersection(&document, &container).expect("the document is on screen");

    assert_eq!(visible, container);
    assert!(!overlaps(&rect(10.0, 1800.0), &visible));
    assert!(overlaps(&rect(10.0, 300.0), &visible));
}

#[test]
fn frames_that_do_not_meet_have_no_visible_part() {
    let first = rect(0.0, 0.0);
    assert_eq!(intersection(&first, &rect(500.0, 0.0)), None);
    assert_eq!(intersection(&first, &rect(0.0, 20.0)), None);
}

#[test]
fn an_element_clamped_to_a_sliver_on_the_viewport_edge_is_off_screen() {
    let viewport = Rect {
        x: 22.0,
        y: 195.0,
        width: 1200.0,
        height: 796.0,
    };
    let clamped = Rect {
        x: 46.0,
        y: 990.0,
        width: 109.0,
        height: 1.0,
    };

    assert!(!overlaps(&clamped, &viewport));
}
