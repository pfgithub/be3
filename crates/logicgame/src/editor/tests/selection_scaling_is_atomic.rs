use super::*;
use crate::editor::canvas::ScaleDirection;

#[test]
fn selection_scaling_is_atomic() {
    let mut editor = LogicEditor::default();
    let two_x = editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: scale(2) },
    );
    let one_x = editor.grid.add_component(
        Point::new(8, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 1,
        },
    );
    editor.selection.components.extend([two_x, one_x]);

    assert!(!editor.scale_selection(ScaleDirection::Down));
    assert_eq!(
        editor.grid.component(two_x).unwrap().kind,
        ComponentKind::Not { scale: scale(2) }
    );
    assert_eq!(
        editor.grid.component(one_x).unwrap().kind,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value: 1,
        }
    );

    assert!(editor.scale_selection(ScaleDirection::Up));
    assert_eq!(
        editor.grid.component(two_x).unwrap().kind,
        ComponentKind::Not { scale: scale(4) }
    );
    assert_eq!(
        editor.grid.component(one_x).unwrap().kind,
        ComponentKind::Storage {
            scale: scale(2),
            value: 1,
        }
    );
}
