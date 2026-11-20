use super::covers;
use agent_desktop_core::Rect;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_maximized_window_covers_a_window_inside_it() {
    let maximized = rect(0.0, 30.0, 1920.0, 983.0);

    assert!(covers(&maximized, &rect(44.0, 74.0, 1200.0, 939.0)));
    assert!(covers(&maximized, &maximized));
}

#[test]
fn a_window_that_reaches_past_the_front_one_still_shows() {
    let maximized = rect(0.0, 30.0, 1920.0, 983.0);

    assert!(!covers(&maximized, &rect(0.0, 141.0, 1200.0, 939.0)));
    assert!(!covers(&maximized, &rect(-10.0, 74.0, 1200.0, 600.0)));
    assert!(!covers(&rect(100.0, 100.0, 400.0, 300.0), &maximized));
}

#[test]
fn a_window_with_no_area_is_never_called_covered() {
    assert!(!covers(
        &rect(0.0, 0.0, 1920.0, 1080.0),
        &rect(10.0, 10.0, 0.0, 0.0)
    ));
}
