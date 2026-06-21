use super::*;
use crate::editor::canvas::RotationDirection;

fn is_snapped(point: Point, scale: Scale) -> bool {
    let scale = scale.get();
    point.x.rem_euclid(scale) == 0 && point.y.rem_euclid(scale) == 0
}

#[test]
fn selection_rotation_rotates_scaled_wire_endpoint_cells() {
    let mut editor = LogicEditor::default();
    let original = wire((0, 0), (4, 0), 2);
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

    assert!(editor.rotate_selection(RotationDirection::Right));

    assert_eq!(editor.grid.wires(), &[wire((0, 0), (0, 4), 2)]);
    assert!(editor
        .selection
        .wire_endpoints
        .iter()
        .all(|endpoint| is_snapped(endpoint.point(), endpoint.wire.scale)));
}
