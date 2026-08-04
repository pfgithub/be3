use super::*;

#[test]
fn canvas_size_is_clamped_to_the_supported_range() {
    let mut builder = GuiBuilder::new();
    GuiBuilder::apply_operation(
        &mut builder,
        &GuiBuilderOperation::SetCanvasSize {
            canvas: GuiCanvasSize::new(10.0, f32::NAN),
        },
    );

    assert_eq!(builder.canvas().width, MIN_CANVAS_SIZE);
    assert_eq!(builder.canvas().height, GuiCanvasSize::default().height);
}
