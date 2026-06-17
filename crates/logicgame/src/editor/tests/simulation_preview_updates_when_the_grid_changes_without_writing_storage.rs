use super::*;

#[test]
fn simulation_preview_updates_when_the_grid_changes_without_writing_storage() {
    let mut editor = LogicEditor::default();
    editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: scale(1),
            value: 1,
        },
    );

    editor.update_simulation_preview();

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
    editor.update_simulation_preview();

    assert_eq!(editor.simulation.steps, 1);
    assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1, 0]);
    assert_eq!(
        editor
            .grid
            .components()
            .filter_map(|component| match component.kind {
                ComponentKind::Storage { value, .. } => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
}
