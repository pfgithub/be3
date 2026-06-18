use super::*;

#[test]
fn storage_state_renders_as_wire_value_rectangle() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Up,
        kind: ComponentKind::Storage {
            scale: Scale::new(4).unwrap(),
            value: 0b1010,
        },
    };

    let triangles = DrawValueTriangle::storage_state(&component, 7);

    assert_eq!(triangles.len(), 2);
    assert!(triangles.iter().all(|triangle| {
        triangle.color == DrawTriangle::WIRE_COLOR
            && triangle.value_index == 7
            && triangle.scale == 4.0
    }));
    assert_eq!(triangles[0].bit_coords, [0.0, 4.0, 0.0]);
    assert_eq!(triangles[1].bit_coords, [0.0, 4.0, 4.0]);
}
