use super::*;

#[test]
fn gui_builder_export_generates_the_designed_code() {
    let mut builder = GuiBuilder::new();
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::Insert {
            location: GuiLocation::new(None, 0),
            widget: GuiWidget::new(GuiWidgetKind::Button {
                text: "Submit".into(),
            }),
        },
    );

    let generated = generate_initial(&builder);
    let code = generated.text_lossy();

    assert_eq!(code, builder.generate_code(None));
    assert!(
        code.contains("if ui.button(\"Submit\").clicked() {"),
        "{code}"
    );
    assert_eq!(artifact_name("Sign up"), "Sign up Code");
}
