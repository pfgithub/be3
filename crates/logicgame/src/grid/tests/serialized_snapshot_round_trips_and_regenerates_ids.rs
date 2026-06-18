use super::*;

#[test]
fn serialized_snapshot_round_trips_and_regenerates_ids() {
    let snapshot = LogicGridSnapshot {
        components: vec![
            Component {
                id: ComponentId(2),
                position: Point::new(-8, 0),
                rotation: Rotation::Up,
                kind: ComponentKind::Not { scale: scale(2) },
            },
            Component {
                id: ComponentId(4),
                position: Point::new(0, 0),
                rotation: Rotation::Right,
                kind: ComponentKind::Led,
            },
            Component {
                id: ComponentId(6),
                position: Point::new(4, 0),
                rotation: Rotation::Down,
                kind: ComponentKind::Storage {
                    scale: scale(4),
                    value: 0b1010,
                },
            },
            Component {
                id: ComponentId(8),
                position: Point::new(8, 0),
                rotation: Rotation::Left,
                kind: ComponentKind::Input {
                    scale: scale(2),
                    id: InputId::from_u128(5),

                    label: String::new(),
                },
            },
            Component {
                id: ComponentId(10),
                position: Point::new(12, 0),
                rotation: Rotation::Up,
                kind: ComponentKind::Output {
                    scale: scale(2),
                    id: OutputId::from_u128(7),

                    label: String::new(),
                },
            },
            Component {
                id: ComponentId(12),
                position: Point::new(16, 0),
                rotation: Rotation::Right,
                kind: ComponentKind::subcomponent(
                    file_id(),
                    component_hash(),
                    Size::new(4, 6),
                    vec![
                        ComponentPort::input(0, scale(2), ComponentSide::Left, 0, 2),
                        ComponentPort::output(0, scale(2), ComponentSide::Right, 2, 4),
                    ],
                )
                .unwrap(),
            },
        ],
        wires: vec![wire((-4, 8), (4, 8), 2)],
    };
    let json = serde_json::to_vec_pretty(&snapshot).unwrap();
    let decoded: LogicGridSnapshot = serde_json::from_slice(&json).unwrap();
    let mut grid = LogicGrid::from_snapshot(decoded);
    assert_eq!(grid.snapshot(), snapshot);
    assert_eq!(grid.revision(), 0);

    let component = grid.add_component(Point::new(0, 12), Rotation::Up, ComponentKind::Led);
    assert_eq!(component, ComponentId(13));
    let input = grid.add_component(
        Point::new(4, 12),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(u128::MAX),

            label: String::new(),
        },
    );
    let ComponentKind::Input { id: input_id, .. } = grid.component(input).unwrap().kind else {
        panic!("expected input");
    };
    assert_ne!(input_id, InputId::from_u128(u128::MAX));
    let output = grid.add_component(
        Point::new(8, 12),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(u128::MAX),

            label: String::new(),
        },
    );
    let ComponentKind::Output { id: output_id, .. } = grid.component(output).unwrap().kind else {
        panic!("expected output");
    };
    assert_ne!(output_id, OutputId::from_u128(u128::MAX));
}
