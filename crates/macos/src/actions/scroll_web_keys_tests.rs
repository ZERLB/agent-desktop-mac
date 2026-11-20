use super::{arrow_key, consumes_arrow_keys, moved_toward, overlaps};
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
