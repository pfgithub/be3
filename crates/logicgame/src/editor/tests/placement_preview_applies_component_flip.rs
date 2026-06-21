use super::*;

#[test]
fn placement_preview_applies_component_flip() {
    let flip = ComponentFlip {
        horizontal: false,
        vertical: true,
    };

    let preview = component_preview(
        Tool {
            kind: ToolKind::Not,
            scale: scale(2),
            merger_out_scale: scale(2),
        },
        Point::new(4, 6),
        Rotation::Right,
        flip,
        None,
    )
    .unwrap();

    assert_eq!(preview.flip, flip);
    assert_eq!(preview.rotation, Rotation::Right);
}
