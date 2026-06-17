use super::*;

#[test]
fn mixed_selection_moves_and_deletes_components_and_wires() {
    let mut editor = LogicEditor::default();
    let component = editor.grid.add_component(
        Point::new(0, 0),
        Rotation::Right,
        ComponentKind::Not { scale: scale(2) },
    );
    let original_wire = wire((0, 8), (16, 8), 8);
    editor.grid.add_wire(original_wire);
    editor.selection.components.insert(component);
    editor.selection.wire_endpoints.extend([
        WireEndpoint {
            wire: original_wire,
            end: WireEnd::Start,
        },
        WireEndpoint {
            wire: original_wire,
            end: WireEnd::End,
        },
    ]);

    let gesture = editor.move_gesture([0.0, 0.0]).unwrap();
    let Gesture::MoveSelection {
        scale: snap_scale,
        components,
        wires,
        ..
    } = gesture
    else {
        panic!("expected a move gesture");
    };
    assert_eq!(snap_scale, scale(8));

    editor.apply_move(&components, &wires, Point::new(8, -8));
    assert_eq!(
        editor.grid.component(component).unwrap().position,
        Point::new(8, -8)
    );
    assert_eq!(editor.grid.wires(), &[wire((8, 0), (24, 0), 8)]);
    assert!(editor.grid.validate().is_empty());

    editor.delete_selection();
    assert!(editor.grid.component(component).is_none());
    assert!(editor.grid.wires().is_empty());
    assert!(editor.selection.is_empty());
}
