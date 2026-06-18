use super::*;

fn assert_position_near(actual: [f32; 2], expected: [f32; 2]) {
    assert!((actual[0] - expected[0]).abs() < 0.00001);
    assert!((actual[1] - expected[1]).abs() < 0.00001);
}

#[test]
fn wires_and_components_are_emitted_as_filled_triangles() {
    let wire = Wire::new(Point::new(0, 0), Point::new(4, 0), Scale::ONE).unwrap();
    let wire_triangles = DrawTriangle::wire(wire, DrawTriangle::WIRE_COLOR);
    assert_eq!(wire_triangles.len(), 6);
    assert!(wire_triangles
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::WIRE_COLOR));
    assert_position_near(wire_triangles[2].positions[0], [0.12, 0.12]);
    assert_position_near(wire_triangles[4].positions[0], [4.12, 0.12]);
    let endpoint_triangles = DrawTriangle::wire_endpoints(wire);
    assert_eq!(endpoint_triangles.len(), 4);
    assert!(endpoint_triangles
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::WIRE_ENDPOINT_COLOR));
    assert_position_near(endpoint_triangles[0].positions[0], [0.12, 0.12]);
    assert_position_near(endpoint_triangles[2].positions[0], [4.12, 0.12]);

    let gate = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Right,
        kind: ComponentKind::Not { scale: Scale::ONE },
    };
    let gate_triangles = DrawTriangle::component(&gate, false);
    assert_eq!(gate_triangles.len(), 13);
    assert!(gate_triangles[..2]
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::COMPONENT_BACKGROUND_COLOR));
    assert!(gate_triangles
        .iter()
        .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
    assert!(gate_triangles
        .iter()
        .any(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));
    let gate_triangle = gate_triangles[12];
    let tip = gate_triangle.positions[2];
    assert!(tip[0] > gate_triangle.positions[0][0]);
    assert!(tip[0] > gate_triangle.positions[1][0]);
    assert_eq!(gate_triangle.color, DrawTriangle::GATE_COLOR);

    let led = Component {
        id: ComponentId(1),
        position: Point::new(0, 0),
        rotation: Rotation::Up,
        kind: ComponentKind::Led,
    };
    let led_triangles = DrawTriangle::component(&led, false);
    assert_eq!(led_triangles.len(), 15);
    assert!(led_triangles
        .iter()
        .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));

    let storage = Component {
        id: ComponentId(2),
        position: Point::new(0, 0),
        rotation: Rotation::Right,
        kind: ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    };
    let storage_triangles = DrawTriangle::component(&storage, false);
    assert_eq!(storage_triangles.len(), 14);
    assert!(storage_triangles
        .iter()
        .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
    assert!(storage_triangles
        .iter()
        .any(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));

    let invalid_gate = DrawTriangle::component(&gate, true);
    assert!(invalid_gate[2..10]
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::ERROR_COLOR));

    let highlight = DrawTriangle::component_highlight(&gate);
    assert_eq!(highlight.len(), 8);
    assert!(highlight
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::HIGHLIGHT_COLOR));

    let connection = DrawTriangle::connection_highlight(
        &gate,
        ConnectionSlot {
            id: ConnectionSlotId(0),
            direction: ConnectionDirection::Input,
            side: ComponentSide::Bottom,
            start: 10,
            end: 12,
            scale: Scale::ONE,
        },
    );
    assert_eq!(connection.len(), 2);
    assert!(connection
        .iter()
        .all(|triangle| triangle.color == DrawTriangle::HIGHLIGHT_COLOR));
}
