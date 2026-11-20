use super::{Standing, standing};

#[test]
fn the_target_in_front_took_the_persons_place() {
    assert_eq!(standing(Some(20), 10, 20), Standing::Taken);
}

#[test]
fn the_persons_application_still_in_front_needs_nothing() {
    assert_eq!(standing(Some(10), 10, 20), Standing::Kept);
}

#[test]
fn a_third_application_in_front_is_the_persons_own_move() {
    assert_eq!(standing(Some(30), 10, 20), Standing::Elsewhere);
}

#[test]
fn an_unknown_front_moves_nothing() {
    assert_eq!(standing(None, 10, 20), Standing::Elsewhere);
}

#[test]
fn a_watch_for_a_process_that_never_comes_forward_puts_nothing_back() {
    let Some(guard) = super::remember() else {
        return;
    };
    assert!(guard.person_is_in(guard.person.pid()));
    let nobody = i32::MAX;
    assert!(!guard.watch_beside(nobody).finish(2));
}

#[test]
fn dropping_a_watch_ends_it() {
    if let Some(guard) = super::remember() {
        drop(guard.watch_beside(i32::MAX));
    }
}
