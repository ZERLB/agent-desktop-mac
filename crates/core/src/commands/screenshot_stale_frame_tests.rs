use super::stale_frame_note;
use crate::adapter::ScreenshotTarget;

fn window(visible: Option<bool>, minimized: Option<bool>) -> ScreenshotTarget {
    ScreenshotTarget::ExactWindow(crate::WindowInfo {
        id: "w-1".into(),
        title: "Document".into(),
        app: "Editor".into(),
        pid: crate::ProcessId::new(7),
        process_instance: Some("generation-7".into()),
        bounds: None,
        state: crate::WindowState {
            visible,
            minimized,
            ..Default::default()
        },
    })
}

#[test]
fn a_window_on_screen_carries_no_note() {
    assert_eq!(stale_frame_note(&window(Some(true), Some(false))), None);
    assert_eq!(stale_frame_note(&window(None, None)), None);
    assert_eq!(stale_frame_note(&ScreenshotTarget::FullScreen), None);
}

#[test]
fn a_window_off_screen_warns_that_the_frame_can_be_old() {
    let note = stale_frame_note(&window(Some(false), None)).expect("a note");

    assert_eq!(note["possible"], true);
    assert_eq!(note["reason"], "window_not_on_screen");
    assert!(
        note["note"]
            .as_str()
            .is_some_and(|text| text.contains("snapshot"))
    );
}

#[test]
fn a_minimized_window_names_its_own_reason() {
    let note = stale_frame_note(&window(Some(false), Some(true))).expect("a note");

    assert_eq!(note["reason"], "window_minimized");
}
