use super::{FOCUS, OTHER, PRESS, patience};
use agent_desktop_core::Action;

#[test]
fn a_press_is_given_time_for_the_window_it_opens() {
    for action in [
        Action::Click,
        Action::DoubleClick,
        Action::Select("Salud".into()),
        Action::Toggle,
    ] {
        assert_eq!(patience(&action), PRESS);
    }
    assert_ne!((PRESS.window_looks, PRESS.app_looks), (0, 0));
}

#[test]
fn a_focus_write_is_watched_for_its_application_only() {
    assert_eq!(patience(&Action::SetFocus), FOCUS);
    assert_eq!(FOCUS.window_looks, 0);
}

#[test]
fn any_other_action_gets_one_last_look() {
    for action in [
        Action::SetValue("7".into()),
        Action::TypeText("hola".into()),
        Action::Clear,
        Action::Hover,
    ] {
        assert_eq!(patience(&action), OTHER);
    }
    assert_eq!(OTHER.window_looks, 0);
}
