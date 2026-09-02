use super::*;

#[test]
fn placement_preview_applies_component_orientation() {
    let preview = component_preview(
        Tool {
            kind: ToolKind::Not,
            scale: scale(2),
            merger_out_scale: scale(2),
        },
        Point::new(4, 6),
        ComponentOrientation::LeftMirrored,
        None,
    )
    .unwrap();

    assert_eq!(preview.orientation, ComponentOrientation::LeftMirrored);
}
