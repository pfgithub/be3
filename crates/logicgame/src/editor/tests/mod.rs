use super::*;
use logicgame::execution::ComponentExecutionSubgraph;
use logicgame::grid::{ComponentSide, ConnectionDirection, ConnectionSlotId};

fn scale(value: u8) -> Scale {
    Scale::new(value).unwrap()
}

fn wire(start: (i64, i64), end: (i64, i64), scale: u8) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::new(scale).unwrap(),
    )
    .unwrap()
}

mod box_selection_finds_components_and_individual_wire_endpoints;
mod gate_drag_maps_to_rotation_and_placement_anchor;
mod graph_hover_maps_each_node_to_its_grid_geometry;
mod instructions_have_compact_display_names;
mod mixed_selection_moves_and_deletes_components_and_wires;
mod moving_one_wire_endpoint_resizes_or_deletes_the_segment;
mod opening_challenge_solution_preserves_challenge_mode;
mod screen_world_round_trip_and_cursor_zoom_are_stable;
mod selection_movement_snaps_to_its_largest_scale;
mod simulation_instruction_view_follows_calls_and_can_select_callers_and_targets;
mod simulation_preview_updates_when_the_grid_changes_without_writing_storage;
mod simulation_restarts_when_the_grid_changes;
mod simulation_tracks_the_next_instruction;
mod snapping_selects_the_containing_grid_cell;
mod square_components_stay_on_the_clicked_cell_for_every_rotation;
mod storage_bits_are_displayed_from_most_to_least_significant;
mod wire_endpoint_hit_testing_ignores_segment_bodies;
mod wire_projection_uses_the_dominant_axis;
