use super::*;

#[test]
fn struct_name_override_replaces_the_title() {
    let mut builder = GuiBuilder::new();
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::SetTitle {
            title: "My Window".into(),
        },
    );

    let titled = builder.generate_code(None);
    assert!(titled.contains("pub struct MyWindow {"), "{titled}");

    let renamed = builder.generate_code(Some("settings panel"));
    assert!(renamed.contains("pub struct SettingsPanel {"), "{renamed}");
    assert!(renamed.contains("impl SettingsPanel {"), "{renamed}");

                                                
    let blank = builder.generate_code(Some("  "));
    assert_eq!(blank, titled);
}
