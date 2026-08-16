use super::*;

#[test]
fn ui_settings_apply_operation_sets_zoom() {
    let mut settings = UiSettings::new();

    UiSettings::apply_operation(&mut settings, &UiSettingsOperation::SetZoom { zoom: 1.5 });

    assert_eq!(settings.zoom(), 1.5);
}
