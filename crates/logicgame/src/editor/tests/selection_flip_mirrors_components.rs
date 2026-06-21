use super::*;
use crate::editor::canvas::FlipDirection;

#[test]
fn selection_flip_mirrors_components() {
    let mut editor = LogicEditor::default();
    let left = editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    let right = editor.grid.add_component(
        Point::new(3, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    editor.selection.components.extend([left, right]);

    assert!(editor.flip_selection(FlipDirection::Horizontal));

    let left = editor.grid.component(left).unwrap();
    let right = editor.grid.component(right).unwrap();
    assert_eq!(left.position, Point::new(3, 0));
    assert_eq!(right.position, Point::new(0, 0));
    assert_eq!(
        left.flip,
        ComponentFlip {
            horizontal: true,
            vertical: false,
        }
    );
    assert_eq!(right.flip, left.flip);
    assert!(editor.grid.validate().is_empty());
}
