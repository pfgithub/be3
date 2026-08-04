use super::*;

#[test]
fn moving_one_wire_endpoint_resizes_or_deletes_the_segment() {
    let mut editor = LogicGridEditor::default();
    let original = wire((0, 0), (8, 0), 1);
    editor.seed(|grid| {
        grid.add_wire(original);
    });
    editor.selection.wire_endpoints.insert(WireEndpoint {
        wire: original,
        end: WireEnd::End,
    });

    let Gesture::MoveSelection { wires, .. } =
        editor.move_gesture([8.5, 0.5]).expect("move gesture")
    else {
        panic!("expected a move gesture");
    };
    editor.apply_move(&[], &wires, Point::new(4, 0));
    assert_eq!(editor.grid.wires(), &[wire((0, 0), (12, 0), 1)]);

    let Gesture::MoveSelection { wires, .. } =
        editor.move_gesture([12.5, 0.5]).expect("move gesture")
    else {
        panic!("expected a move gesture");
    };
    editor.apply_move(&[], &wires, Point::new(-6, 0));
    assert_eq!(editor.grid.wires(), &[wire((0, 0), (6, 0), 1)]);

    let Gesture::MoveSelection { wires, .. } =
        editor.move_gesture([6.5, 0.5]).expect("move gesture")
    else {
        panic!("expected a move gesture");
    };
    editor.apply_move(&[], &wires, Point::new(0, 1));
    assert!(editor.grid.wires().is_empty());
    assert!(editor.selection.is_empty());
}
