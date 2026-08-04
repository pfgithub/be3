use super::super::canvas::RotationDirection;
use super::*;

#[test]
fn selection_rotation_rotates_components() {
    let mut editor = LogicGridEditor::default();
    let component = editor.seed(|grid| {
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: scale(2),
                id: InputId::from_u128(0),
                label: String::new(),
            },
        )
    });
    editor.selection.components.insert(component);

    assert!(editor.rotate_selection(RotationDirection::Right));
    let component = editor.grid.component(component).unwrap();
    assert_eq!(component.position, Point::new(0, 0));
    assert_eq!(component.orientation, ComponentOrientation::Right);
}
