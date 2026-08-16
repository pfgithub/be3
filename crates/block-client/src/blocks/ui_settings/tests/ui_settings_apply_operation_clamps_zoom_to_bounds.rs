use super::*;

#[test]
fn ui_settings_apply_operation_clamps_zoom_to_bounds() {
    let mut settings = UiSettings::new();

    UiSettings::apply_operation(&mut settings, &UiSettingsOperation::SetZoom { zoom: 10.0 });
    assert_eq!(settings.zoom(), 3.0);

    UiSettings::apply_operation(&mut settings, &UiSettingsOperation::SetZoom { zoom: -1.0 });
    assert_eq!(settings.zoom(), 0.5);
}
