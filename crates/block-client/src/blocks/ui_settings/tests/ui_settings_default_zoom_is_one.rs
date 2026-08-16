use super::*;

#[test]
fn ui_settings_default_zoom_is_one() {
    assert_eq!(UiSettings::new().zoom(), 1.0);
}
