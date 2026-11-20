use super::placement;
use agent_desktop_core::{Point, Rect};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

const SCREEN: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1920.0,
    height: 1080.0,
};

#[test]
fn a_window_behind_a_maximized_one_slides_down_to_the_band_beside_the_dock() {
    let maximized = rect(0.0, 30.0, 1920.0, 983.0);
    let back = rect(44.0, 74.0, 1200.0, 939.0);

    assert_eq!(
        placement(&back, &maximized, &SCREEN),
        Some(Point { x: 44.0, y: 141.0 })
    );
}

#[test]
fn the_widest_free_band_wins() {
    let front = rect(0.0, 30.0, 1400.0, 1040.0);
    let back = rect(100.0, 100.0, 800.0, 600.0);

    assert_eq!(
        placement(&back, &front, &SCREEN),
        Some(Point {
            x: 1120.0,
            y: 100.0
        })
    );
}

#[test]
fn a_front_window_that_fills_the_screen_leaves_nowhere_to_go() {
    let front = rect(0.0, 0.0, 1920.0, 1080.0);

    assert_eq!(
        placement(&rect(44.0, 74.0, 1200.0, 939.0), &front, &SCREEN),
        None
    );
}

#[test]
fn a_window_taller_than_the_screen_is_not_pushed_above_it() {
    let maximized = rect(0.0, 30.0, 1920.0, 983.0);

    assert_eq!(
        placement(&rect(44.0, 30.0, 1200.0, 1200.0), &maximized, &SCREEN),
        None
    );
}

#[test]
fn a_second_screen_keeps_its_own_coordinates() {
    let screen = rect(-215.0, -1080.0, 1920.0, 1080.0);
    let front = rect(-215.0, -1050.0, 1920.0, 983.0);
    let back = rect(-100.0, -1000.0, 1000.0, 700.0);

    assert_eq!(
        placement(&back, &front, &screen),
        Some(Point {
            x: -100.0,
            y: -700.0
        })
    );
}
