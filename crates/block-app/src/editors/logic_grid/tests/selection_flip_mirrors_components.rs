use super::super::canvas::FlipDirection;
use super::*;

#[test]
fn selection_flip_mirrors_components() {
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
            Point::new(3, 0),
            Rotation::Up,
            ComponentKind::Not { scale: Scale::ONE },
        )
    });
    editor.selection.components.extend([left, right]);

    assert!(editor.flip_selection(FlipDirection::Horizontal));

    let left = editor.grid.component(left).unwrap();
    let right = editor.grid.component(right).unwrap();
    assert_eq!(left.position, Point::new(3, 0));
    assert_eq!(right.position, Point::new(0, 0));
    assert_eq!(left.orientation, ComponentOrientation::UpMirrored);
    assert_eq!(right.orientation, left.orientation);
    assert!(editor.grid.validate().is_empty());
}
