use super::*;

#[test]
fn inserting_a_component_keeps_its_id_and_ignores_repeats() {
    let mut grid = LogicGrid::new();
    let id = grid.next_component_id();
    let led = Component {
        id,
        position: Point::new(0, 0),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Led,
    };

    assert!(grid.insert_component(led.clone()));
    assert_eq!(grid.component(id), Some(&led));

    assert_ne!(grid.next_component_id(), id);

    let revision = grid.revision();
    assert!(!grid.insert_component(led));
    assert_eq!(grid.revision(), revision);
    assert_eq!(grid.components().count(), 1);
}
