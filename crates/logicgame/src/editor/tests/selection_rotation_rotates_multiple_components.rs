use super::*;
use crate::editor::canvas::RotationDirection;

#[test]
fn selection_rotation_rotates_multiple_components() {
    let mut editor = LogicEditor::default();
    let left = editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let right = editor.grid.add_component(
        Point::new(2, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    editor.selection.components.extend([left, right]);

    assert!(editor.rotate_selection(RotationDirection::Right));

    let left = editor.grid.component(left).unwrap();
    let right = editor.grid.component(right).unwrap();
    assert_eq!(left.position, Point::new(0, 0));
    assert_eq!(left.rotation, Rotation::Right);
    assert_eq!(right.position, Point::new(0, 2));
    assert_eq!(right.rotation, Rotation::Right);
    assert!(editor.grid.validate().is_empty());
}
