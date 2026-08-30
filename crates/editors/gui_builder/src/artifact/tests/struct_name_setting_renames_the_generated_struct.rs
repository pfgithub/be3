use super::*;

#[test]
fn struct_name_setting_renames_the_generated_struct() {
    let mut builder = GuiBuilder::new();
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::SetTitle {
            title: "My Window".into(),
        },
    );

    let source = uuid::Uuid::new_v4();
    let data = descriptor(source).data;
    let artifact = CodeArtifact::decode(&data).unwrap();
    assert_eq!(artifact.source, source);
    assert_eq!(artifact.settings, CodeSettings::default());
    assert!(generate_initial(&builder)
        .text_lossy()
        .contains("pub struct MyWindow {"));

    let settings = CodeSettings {
        struct_name: "Settings Panel".into(),
        rename_with_source: false,
    };
    let generated = generate(&builder, &settings);
    let code = generated.text_lossy();

    assert!(code.contains("pub struct SettingsPanel {"), "{code}");
}
