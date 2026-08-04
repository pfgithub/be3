use super::*;

#[test]
fn gui_builder_serialization_round_trips() {
    let mut builder = GuiBuilder::new();
    let group = insert(&mut builder, None, 0, container());
    insert(
        &mut builder,
        Some(group),
        0,
        GuiWidget::new(GuiWidgetKind::Slider {
            label: "Volume".into(),
            value: 0.5,
            min: 0.0,
            max: 1.0,
        }),
    );
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::SetTitle {
            title: "Mixer".into(),
        },
    );

    let json = serde_json::to_string(&builder).unwrap();
    assert_eq!(serde_json::from_str::<GuiBuilder>(&json).unwrap(), builder);
}
