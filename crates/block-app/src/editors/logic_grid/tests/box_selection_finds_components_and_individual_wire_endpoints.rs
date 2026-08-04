use super::*;

#[test]
fn box_selection_finds_components_and_individual_wire_endpoints() {
    let mut editor = LogicGridEditor::default();
    let inside = editor.seed(|grid| {
        grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        )
    });
    let outside = editor.seed(|grid| {
        grid.add_component(
            Point::new(20, 20),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        )
    });
    let selected_wire = wire((0, 6), (8, 6), 2);
    editor.seed(|grid| {
        grid.add_wire(selected_wire);
    });

    editor.select_in_rect([-1.0, -1.0], [2.5, 8.0]);

    assert!(editor.selection.components.contains(&inside));
    assert!(!editor.selection.components.contains(&outside));
    assert!(editor.selection.wire_endpoints.contains(&WireEndpoint {
        wire: selected_wire,
        end: WireEnd::Start,
    }));
    assert!(!editor.selection.wire_endpoints.contains(&WireEndpoint {
        wire: selected_wire,
        end: WireEnd::End,
    }));
}
