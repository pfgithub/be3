use super::super::canvas::RotationDirection;
use super::*;

fn is_snapped(point: Point, scale: Scale) -> bool {
    let scale = scale.get();
    point.x.rem_euclid(scale) == 0 && point.y.rem_euclid(scale) == 0
}

#[test]
fn selection_rotation_keeps_scaled_components_on_their_grid() {
    let mut editor = LogicGridEditor::default();
    let one_x = editor
        .grid
        .add_component(Point::new(1, 0), Rotation::Up, ComponentKind::Led);
    let two_x = editor.seed(|grid| {
        grid.add_component(
            Point::new(4, 0),
            Rotation::Up,
            ComponentKind::Not { scale: scale(2) },
        )
    });
    editor.selection.components.extend([one_x, two_x]);

    assert!(editor.rotate_selection(RotationDirection::Right));

    let component = editor.grid.component(two_x).unwrap();
    assert!(is_snapped(component.position, component.kind.snap()));
    assert!(editor.grid.validate().is_empty());
}
