use super::*;

#[test]
fn viewport_grid_and_entities_are_layered_and_bounded() {
    let entity = DrawTriangle::new(
        [[1.0, 1.0], [2.0, 1.0], [1.0, 2.0]],
        DrawTriangle::WIRE_COLOR,
    );
    let frame = RenderFrame {
        viewport_size: [100.0, 100.0],
        camera_center: [0.0, 0.0],
        zoom: 10.0,
        grid_scale: 1.0,
        wires: Vec::new(),
        wire_values: Vec::new(),
        rays: Vec::new(),
        stubs: Vec::new(),
        value_triangles: Vec::new(),
        triangles: vec![entity],
    };

    let triangles = frame_triangles(&frame);
    assert_eq!(triangles[0].color, BACKGROUND_COLOR);
    assert_eq!(triangles.last().unwrap().color, DrawTriangle::WIRE_COLOR);

    for triangle in triangles.iter().filter(|triangle| {
        triangle.color == MINOR_GRID_COLOR
            || triangle.color == MAJOR_GRID_COLOR
            || triangle.color == AXIS_COLOR
    }) {
        assert!(triangle
            .positions
            .iter()
            .all(|[x, y]| { -5.0 <= *x && *x <= 5.0 && -5.0 <= *y && *y <= 5.0 }));
    }
}
