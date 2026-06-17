use super::*;

#[test]
fn connection_indicator_sits_on_the_wired_port_boundary() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Up,
        kind: ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    };
    let radius = Scale::ONE.get() as f32 * 0.28;

    for (side, start, end, boundary_center) in [
        (ComponentSide::Top, 10, 11, [10.5, 20.0]),
        (ComponentSide::Right, 20, 22, [11.0, 21.0]),
        (ComponentSide::Bottom, 10, 11, [10.5, 22.0]),
        (ComponentSide::Left, 20, 22, [10.0, 21.0]),
    ] {
        for direction in [ConnectionDirection::Input, ConnectionDirection::Output] {
            let triangles = DrawTriangle::connection_indicator(
                &component,
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction,
                    side,
                    start,
                    end,
                    scale: Scale::ONE,
                },
            );

            let expected_color = match direction {
                ConnectionDirection::Input => DrawTriangle::INPUT_COLOR,
                ConnectionDirection::Output => DrawTriangle::OUTPUT_COLOR,
            };
            assert_eq!(triangles.len(), 4);
            for triangle in &triangles {
                assert_eq!(triangle.color, expected_color);
                assert_eq!(triangle.positions[0], boundary_center);
                for [x, y] in triangle.positions {
                    assert!((x - boundary_center[0]).abs() <= radius + 1e-6);
                    assert!((y - boundary_center[1]).abs() <= radius + 1e-6);
                }
            }
        }
    }
}
