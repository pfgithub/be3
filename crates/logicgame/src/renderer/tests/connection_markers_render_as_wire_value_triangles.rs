use super::*;

#[test]
fn connection_markers_render_as_wire_value_triangles() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Up,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Storage {
            scale: Scale::new(4).unwrap(),
            value: 0,
        },
    };
    let connection = ConnectionSlot {
        id: ConnectionSlotId(0),
        direction: ConnectionDirection::Output,
        side: ComponentSide::Top,
        start: 10,
        end: 14,
        scale: Scale::new(4).unwrap(),
    };

    let marker = DrawValueTriangle::connection_marker(&component, connection, 1.6, 5);

    assert_eq!(marker.color, DrawTriangle::OUTPUT_COLOR);
    assert_eq!(marker.value_index, 5);
    assert_eq!(marker.scale, 4.0);
    assert!(marker.bit_coords.iter().any(|&bit| bit == 0.0));
    assert!(marker
        .bit_coords
        .iter()
        .any(|&bit| (bit - 4.0).abs() < 0.00001));
}
