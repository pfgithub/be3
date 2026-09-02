use super::*;

#[test]
fn merger_splitter_renders_order_lines() {
    let splitter = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        orientation: ComponentOrientation::Right,
        kind: ComponentKind::MergerSplitter {
            input_scale: Scale::new(16).unwrap(),
            output_scale: Scale::new(4).unwrap(),
        },
    };

    let triangles = DrawTriangle::component(&splitter, false);
    assert_eq!(triangles.len(), 23);
    let order_lines = &triangles[15..];
    assert!(order_lines
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::GATE_COLOR));

    let first_line = bbox(&order_lines[..2]);
    let last_line = bbox(&order_lines[6..8]);
    assert!(first_line[1] < 2.15 && first_line[3] > 1.85);
    assert!(last_line[1] < 14.15 && last_line[3] > 13.85);
}
