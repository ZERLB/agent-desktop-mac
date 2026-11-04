use super::CursorLook;
use crate::CursorOverlayStyle;

#[test]
fn a_look_is_named_solid_or_glass_in_any_case() {
    assert_eq!(CursorLook::parse("glass"), Some(CursorLook::Glass));
    assert_eq!(CursorLook::parse("GLASS"), Some(CursorLook::Glass));
    assert_eq!(CursorLook::parse("Solid"), Some(CursorLook::Solid));
    assert_eq!(CursorLook::parse("neon"), None);
}

#[test]
fn the_solid_look_stays_out_of_the_saved_style() {
    let saved = serde_json::to_value(CursorOverlayStyle::default()).unwrap();

    assert!(saved.get("look").is_none());
}

#[test]
fn a_glass_style_round_trips_and_an_older_style_reads_as_solid() {
    let mut style = CursorOverlayStyle::default();
    style.set_look(CursorLook::Glass);
    let saved = serde_json::to_value(&style).unwrap();

    assert_eq!(saved["look"], "glass");
    let restored: CursorOverlayStyle = serde_json::from_value(saved).unwrap();
    assert!(restored.look().is_glass());

    let older: CursorOverlayStyle =
        serde_json::from_value(serde_json::json!({ "rim": "#111318", "size": 1.0 })).unwrap();
    assert!(older.look().is_solid());
    assert!(
        serde_json::from_value::<CursorOverlayStyle>(serde_json::json!({ "look": "neon" }))
            .is_err()
    );
}
