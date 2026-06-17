use super::*;

#[test]
fn simulation_restarts_when_the_grid_changes() {
    let mut editor = LogicEditor::default();
    editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(1),
            value: 1,
        },
    );

    editor.run_simulation_tick();
    assert_eq!(editor.simulation.steps, 1);
    assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1]);

    editor.grid.add_component(
        Point::new(4, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(1),
            value: 0,
        },
    );
    editor.run_simulation_tick();

    assert_eq!(editor.simulation.steps, 1);
    assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1, 0]);
}
