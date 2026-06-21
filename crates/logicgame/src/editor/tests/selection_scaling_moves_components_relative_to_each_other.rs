use super::*;
use crate::editor::canvas::ScaleDirection;

#[test]
fn selection_scaling_moves_components_relative_to_each_other() {
    let mut editor = LogicEditor::default();
    let left = editor.grid.add_component(
        Point::new(1, 1),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let right = editor.grid.add_component(
        Point::new(5, 1),
        Rotation::Up,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 1,
        },
    );
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
