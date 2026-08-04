use super::*;

#[test]
fn generated_code_binds_stateful_widgets_to_struct_fields() {
    let mut builder = GuiBuilder::new();
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::SetTitle {
            title: "Sign up".into(),
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::Heading {
            text: "Sign up".into(),
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::TextField {
            label: "Full name".into(),
            value: String::new(),
            multiline: false,
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::Checkbox {
            label: "I agree".into(),
            checked: true,
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::Slider {
            label: "Volume".into(),
            value: 0.25,
            min: 0.0,
            max: 1.0,
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::Button {
            text: "Submit".into(),
        },
    );

    let code = builder.generate_code(None);
    assert!(code.contains("pub struct SignUp {"), "{code}");
    assert!(code.contains("pub full_name: String,"), "{code}");
    assert!(code.contains("pub i_agree: bool,"), "{code}");
    assert!(code.contains("pub volume: f32,"), "{code}");
    assert!(code.contains("full_name: String::new(),"), "{code}");
    assert!(code.contains("i_agree: true,"), "{code}");
    assert!(code.contains("volume: 0.25,"), "{code}");
    assert!(
        code.contains("ui.text_edit_singleline(&mut self.full_name);"),
        "{code}"
    );
    assert!(
        code.contains("ui.checkbox(&mut self.i_agree, \"I agree\");"),
        "{code}"
    );
    assert!(
        code.contains("ui.add(egui::Slider::new(&mut self.volume, 0.0..=1.0).text(\"Volume\"));"),
        "{code}"
    );
    assert!(
        code.contains("if ui.button(\"Submit\").clicked() {"),
        "{code}"
    );
}
