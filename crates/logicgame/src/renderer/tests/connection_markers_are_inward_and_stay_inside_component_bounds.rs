use super::*;

#[test]
fn connection_markers_are_inward_and_stay_inside_component_bounds() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Up,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Storage {
            scale: Scale::ONE,
            value: 0,
        },
    };
    let radius = 0.2;

    for (side, start, end) in [
        (ComponentSide::Top, 10, 11),
        (ComponentSide::Right, 20, 22),
        (ComponentSide::Bottom, 10, 11),
        (ComponentSide::Left, 20, 22),
    ] {
        let inward = match side {
            ComponentSide::Top => [0.0, radius],
            ComponentSide::Right => [-radius, 0.0],
            ComponentSide::Bottom => [0.0, -radius],
            ComponentSide::Left => [radius, 0.0],
        };
        let boundary_center = match side {
            ComponentSide::Top => [10.5, 20.0],
            ComponentSide::Right => [11.0, 21.0],
            ComponentSide::Bottom => [10.5, 22.0],
            ComponentSide::Left => [10.0, 21.0],
        };
        let inside_center = [
            boundary_center[0] + inward[0],
            boundary_center[1] + inward[1],
        ];

        for direction in [ConnectionDirection::Input, ConnectionDirection::Output] {
            let marker = connection_marker(
                &component,
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction,
                    side,
                    start,
                    end,
                    scale: Scale::ONE,
                },
                radius,
            )[0];

            let expected_tip = match direction {
                ConnectionDirection::Input => inside_center,
                ConnectionDirection::Output => boundary_center,
            };
            assert_eq!(marker.positions[0], expected_tip);
            assert!(marker
                .positions
                .iter()
                .all(|[x, y]| { (10.0..=11.0).contains(x) && (20.0..=22.0).contains(y) }));
        }
    }
}
