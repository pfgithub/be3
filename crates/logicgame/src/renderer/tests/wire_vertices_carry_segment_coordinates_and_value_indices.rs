use super::*;

#[test]
fn wire_vertices_carry_segment_coordinates_and_value_indices() {
    let wire = Wire::new(Point::new(0, 0), Point::new(8, 0), Scale::new(4).unwrap()).unwrap();
    let frame = RenderFrame {
        viewport_size: [80.0, 80.0],
        camera_center: [0.0, 0.0],
        zoom: 10.0,
        grid_scale: 1.0,
        wires: Vec::new(),
        wire_values: vec![WireValue::new(0b1010)],
        rays: Vec::new(),
        stubs: Vec::new(),
        triangles: Vec::new(),
    };
    let vertices = wire_vertices(&[DrawWire::new(wire, DrawTriangle::WIRE_COLOR, 7)], &frame);

    assert_eq!(vertices.len(), 6);
    assert_eq!(vertices[0].bit_coord, 0.0);
    assert_eq!(vertices[2].bit_coord, 4.0);
    assert!(vertices
        .iter()
        .all(|vertex| vertex.value_index == 7 && vertex.scale == 4.0));
    assert_eq!(frame.wire_values[0].low, 0b1010);
    assert_eq!(frame.wire_values[0].high, 0);
}
