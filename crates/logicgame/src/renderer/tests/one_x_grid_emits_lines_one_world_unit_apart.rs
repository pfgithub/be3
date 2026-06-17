use super::*;

#[test]
fn one_x_grid_emits_lines_one_world_unit_apart() {
    let frame = RenderFrame {
        viewport_size: [80.0, 80.0],
        camera_center: [0.0, 0.0],
        zoom: 10.0,
        grid_scale: 1.0,
        wires: Vec::new(),
        wire_values: Vec::new(),
        rays: Vec::new(),
        triangles: Vec::new(),
    };
    let triangles = frame_triangles(&frame);

    let first_minor_x = triangles[4].positions[0][0];
    let second_minor_x = triangles[6].positions[0][0];
    assert!((second_minor_x - first_minor_x - 1.0).abs() < 0.0001);
    assert_eq!(triangles[4].color, MINOR_GRID_COLOR);
    assert_eq!(triangles[6].color, MINOR_GRID_COLOR);
}
