use super::super::canvas::ScaleDirection;
use super::*;

#[test]
fn selection_scaling_moves_components_relative_to_each_other() {
    let mut editor = LogicGridEditor::default();
    let left = editor.seed(|grid| {
        grid.add_component(
            Point::new(1, 1),
            Rotation::Up,
            ComponentKind::Not { scale: Scale::ONE },
        )
    });
    let right = editor.seed(|grid| {
        grid.add_component(
            Point::new(5, 1),
            Rotation::Up,
            ComponentKind::Storage {
                scale: Scale::ONE,
                value: 1,
            },
        )
    });
    editor.selection.components.extend([left, right]);

    assert!(editor.scale_selection(ScaleDirection::Up));

    let left = editor.grid.component(left).unwrap();
    let right = editor.grid.component(right).unwrap();
    assert_eq!(left.position, Point::new(2, 2));
    assert_eq!(right.position, Point::new(10, 2));
    assert_eq!(left.kind.snap(), scale(2));
    assert_eq!(right.kind.snap(), scale(2));
    assert!(editor.grid.validate().is_empty());
}
