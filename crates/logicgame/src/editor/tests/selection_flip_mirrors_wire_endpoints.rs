use super::*;
use crate::editor::canvas::FlipDirection;

#[test]
fn selection_flip_mirrors_wire_endpoints() {
    let mut editor = LogicEditor::default();
    let original = wire((0, 0), (6, 0), 2);
    editor.grid.add_wire(original);
    editor.selection.wire_endpoints.extend([
        WireEndpoint {
            wire: original,
            end: WireEnd::Start,
        },
        WireEndpoint {
            wire: original,
            end: WireEnd::End,
        },
    ]);

    assert!(editor.flip_selection(FlipDirection::Horizontal));

    assert_eq!(editor.grid.wires(), &[original]);
    assert!(editor.selection.wire_endpoints.contains(&WireEndpoint {
        wire: original,
        end: WireEnd::Start,
    }));
    assert!(editor.selection.wire_endpoints.contains(&WireEndpoint {
        wire: original,
        end: WireEnd::End,
    }));
}
