use super::*;

#[test]
fn generated_code_nests_container_children() {
    let mut builder = GuiBuilder::new();
    let row = push(
        &mut builder,
        GuiWidgetKind::Container {
            layout: GuiLayout::Horizontal,
            framed: true,
        },
    );
    insert(
        &mut builder,
        Some(row),
        0,
        GuiWidget::new(GuiWidgetKind::Label {
            text: "Inside".into(),
        }),
    );

    let code = builder.generate_code(None);
    assert!(
        code.contains("        egui::Frame::group(ui.style()).show(ui, |ui| {\n"),
        "{code}"
    );
    assert!(
        code.contains("            ui.horizontal(|ui| {\n"),
        "{code}"
    );
    assert!(
        code.contains("                ui.label(\"Inside\");\n"),
        "{code}"
    );
}
