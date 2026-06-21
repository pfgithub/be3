use super::*;

#[test]
fn connection_stub_extends_outward_from_the_wired_port() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    };
    let frame = RenderFrame {
        viewport_size: [80.0, 80.0],
        camera_center: [0.0, 0.0],
        zoom: 10.0,
        grid_scale: 1.0,
        wires: Vec::new(),
        wire_values: vec![WireValue::new(0b1)],
        rays: Vec::new(),
        stubs: Vec::new(),
        value_triangles: Vec::new(),
        triangles: Vec::new(),
    };

    // Same width as a regular wire and reaching the flush wire's centerline.
    let scale = Scale::ONE.get() as f32;
    let half_thickness = (scale * 0.36).max(0.08);
    let length = scale * 0.5;

    for (side, start, end, boundary_center) in [
        (ComponentSide::Top, 10, 11, [10.5, 20.0]),
        (ComponentSide::Right, 20, 22, [11.0, 21.0]),
        (ComponentSide::Bottom, 10, 11, [10.5, 22.0]),
        (ComponentSide::Left, 20, 22, [10.0, 21.0]),
    ] {
        let [cx, cy] = boundary_center;
        let (left, top, right, bottom) = match side {
            ComponentSide::Top => (cx - half_thickness, cy - length, cx + half_thickness, cy),
            ComponentSide::Bottom => (cx - half_thickness, cy, cx + half_thickness, cy + length),
            ComponentSide::Left => (cx - length, cy - half_thickness, cx, cy + half_thickness),
            ComponentSide::Right => (cx, cy - half_thickness, cx + length, cy + half_thickness),
        };
        let tl = world_to_clip([left, top], &frame);
        let tr = world_to_clip([right, top], &frame);
        let bl = world_to_clip([left, bottom], &frame);
        let br = world_to_clip([right, bottom], &frame);

        // Direction is irrelevant to the stub; it always renders as a wire.
        for direction in [ConnectionDirection::Input, ConnectionDirection::Output] {
            let stub = DrawStub::for_connection(
                &component,
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction,
                    side,
                    start,
                    end,
                    scale: Scale::ONE,
                },
                DrawTriangle::WIRE_COLOR,
                3,
            );
            let vertices = stub_vertices(&[stub], &frame);

            // Two triangles spanning the boundary and the wire centerline.
            assert_eq!(vertices.len(), 6);
            assert_eq!(
                vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                vec![tl, tr, bl, bl, tr, br],
            );

            // Carries the connected net's value so it shows the live on/off
            // state, and is drawn in the wire color.
            assert!(vertices.iter().all(|v| v.value_index == 3
                && v.scale == scale
                && v.fill_color == DrawTriangle::WIRE_COLOR));

            // bit_coord runs across the stub's width, from 0 to scale.
            let bits: Vec<f32> = vertices.iter().map(|v| v.bit_coord).collect();
            assert!(bits.iter().any(|&b| b == 0.0));
            assert!(bits.iter().any(|&b| b == scale));
        }
    }
}
