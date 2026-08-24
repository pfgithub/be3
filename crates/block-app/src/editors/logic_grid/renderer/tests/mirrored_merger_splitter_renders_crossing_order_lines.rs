use super::*;

#[test]
fn mirrored_merger_splitter_renders_crossing_order_lines() {
    let splitter = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        orientation: ComponentOrientation::UpMirrored,
        kind: ComponentKind::MergerSplitter {
            input_scale: Scale::new(16).unwrap(),
            output_scale: Scale::new(4).unwrap(),
        },
    };

    let triangles = DrawTriangle::component(&splitter, false);
    assert_eq!(triangles.len(), 23);
    let order_lines = &triangles[15..];

    let first_line = bbox(&order_lines[..2]);
    let last_line = bbox(&order_lines[6..8]);
    assert!(first_line[0] < 2.15 && first_line[2] > 13.85);
    assert!(last_line[0] < 2.15 && last_line[2] > 13.85);
}
