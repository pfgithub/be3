use super::super::canvas::RotationDirection;
use super::*;

#[test]
fn selection_rotation_rotates_multiple_components() {
    let mut editor = LogicGridEditor::default();
    let left = editor.seed(|grid| {
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Not { scale: Scale::ONE },
        )
    });
    let right = editor.seed(|grid| {
        grid.add_component(
            Point::new(2, 0),
            Rotation::Up,
            ComponentKind::Not { scale: Scale::ONE },
        )
    });
    editor.selection.components.extend([left, right]);

    assert!(editor.rotate_selection(RotationDirection::Right));

    let left = editor.grid.component(left).unwrap();
    let right = editor.grid.component(right).unwrap();
    assert_eq!(left.position, Point::new(0, 0));
    assert_eq!(left.orientation, ComponentOrientation::Right);
    assert_eq!(right.position, Point::new(0, 2));
    assert_eq!(right.orientation, ComponentOrientation::Right);
    assert!(editor.grid.validate().is_empty());
}
