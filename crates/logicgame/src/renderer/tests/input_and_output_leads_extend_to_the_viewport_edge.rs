use super::*;

#[test]
fn input_and_output_leads_extend_to_the_viewport_edge() {
    let input = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        rotation: Rotation::Up,
        kind: ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),

            label: String::new(),
        },
    };
    let output = Component {
        id: ComponentId(1),
        position: Point::new(-10, 0),
        rotation: Rotation::Right,
        kind: ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(0),

            label: String::new(),
        },
    };

    let frame = RenderFrame {
        viewport_size: [100.0, 100.0],
        camera_center: [0.0, 0.0],
        zoom: 2.0,
        grid_scale: 1.0,
        wires: Vec::new(),
        wire_values: Vec::new(),
        rays: vec![
            DrawRay::from_component(&input, DrawTriangle::INPUT_COLOR, 0).unwrap(),
            DrawRay::from_component(&output, DrawTriangle::OUTPUT_COLOR, 0).unwrap(),
        ],
        stubs: Vec::new(),
        value_triangles: Vec::new(),
        triangles: Vec::new(),
    };

    let vertices = ray_vertices(&frame.rays, &frame);
    assert_eq!(vertices.len(), 12);

    let input_verts = &vertices[..6];
    assert!(input_verts
        .iter()
        .all(|v| v.fill_color == DrawTriangle::INPUT_COLOR));
    // input rotated Up → Top lead, ray extends to viewport top
    // viewport top in world = 0.0 - (100.0 / 2.0 / 2.0) = -25.0, clipped to vp_top
    let top_y_clip = input_verts
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        top_y_clip > 0.0,
        "input ray should reach viewport top edge (positive clip y)"
    );

    let output_verts = &vertices[6..];
    assert!(output_verts
        .iter()
        .all(|v| v.fill_color == DrawTriangle::OUTPUT_COLOR));
    // output rotated Right → Right lead, ray extends to viewport right
    let right_x_clip = output_verts
        .iter()
        .map(|v| v.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        right_x_clip > 0.0,
        "output ray should reach viewport right edge (positive clip x)"
    );
}
