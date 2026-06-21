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

/// An editor opened on the NOR challenge with a correct solution: inputs `A`
/// (port 0) and `B` (port 1) drive a shared wired-OR net feeding a NOT gate
/// whose output reaches the `OUT` port (port 0). The grid validates cleanly and
/// computes NOR.
fn nor_challenge_editor() -> LogicEditor {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    grid.add_component_with_explicit_io(
        Point::new(0, -1),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(0),

            label: String::new(),
        },
    );
    grid.add_component_with_explicit_io(
        Point::new(1, 1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(0),

            label: String::new(),
        },
    );
    grid.add_component_with_explicit_io(
        Point::new(2, 1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(1),

            label: String::new(),
        },
    );
    grid.add_wire(wire((0, 2), (3, 2), 1));

    let mut editor = LogicEditor::default();
    editor.open_challenge_solution(ChallengeId::Nor, grid);
    editor
}

mod box_selection_finds_components_and_individual_wire_endpoints;
mod challenge_input_placement_uses_first_missing_port;
mod challenge_input_tool_selects_when_ports_run_out;
mod challenge_output_placement_uses_first_missing_port;
mod challenge_output_tool_selects_when_ports_run_out;
mod challenge_port_slots_are_disabled_when_ports_run_out;
mod challenge_port_slots_map_each_port_to_its_dense_slot;
mod challenge_test_flags_wrong_outputs;
mod challenge_test_passes_for_a_correct_nor_solution;
mod challenge_test_uses_simulation_vm;
mod component_preview_shortcuts;
mod default_hotbar_uses_requested_folder_layout;
mod editing_resets_challenge_test_results;
mod gate_drag_maps_to_rotation_and_placement_anchor;
mod graph_hover_maps_each_node_to_its_grid_geometry;
mod hotbar_drag_down_to_later_slot_moves_after_target;
mod hotbar_drag_into_open_folder_row_moves_items_into_folder;
mod hotbar_drag_moves_items_out_of_folders;
mod hotbar_drag_onto_folder_icon_reorders_before_folder;
mod hotbar_folder_removal_respects_unremovable_items;
mod hotbar_reset_restores_default_layout;
mod hotbar_rows_show_parent_and_open_folder;
mod hotbar_selection_closes_open_folder_when_selecting_outside_it;
mod hotbar_selection_reads_nested_custom_components;
mod instructions_have_compact_display_names;
mod mixed_selection_moves_and_deletes_components_and_wires;
mod moving_one_wire_endpoint_resizes_or_deletes_the_segment;
mod opening_challenge_solution_preserves_challenge_mode;
mod placement_preview_applies_component_orientation;
mod screen_world_round_trip_and_cursor_zoom_are_stable;
mod selection_flip_mirrors_components;
mod selection_flip_mirrors_wire_endpoints;
mod selection_movement_snaps_to_its_largest_scale;
mod selection_rotation_keeps_scaled_components_on_their_grid;
mod selection_rotation_rotates_components;
mod selection_rotation_rotates_multiple_components;
mod selection_rotation_rotates_scaled_wire_endpoint_cells;
mod selection_scaling_is_atomic;
mod selection_scaling_moves_components_relative_to_each_other;
mod simulation_instruction_view_follows_calls_and_can_select_callers_and_targets;
mod simulation_preview_updates_when_the_grid_changes_without_writing_storage;
mod simulation_restarts_when_the_grid_changes;
mod simulation_tracks_the_next_instruction;
mod snapping_selects_the_containing_grid_cell;
mod square_components_stay_on_the_clicked_cell_for_every_rotation;
mod storage_bits_are_displayed_from_most_to_least_significant;
mod subcomponent_placement_keeps_the_anchor_under_the_cursor;
mod wire_deletion_uses_pointer_position_at_intersections;
mod wire_endpoint_hit_testing_ignores_segment_bodies;
mod wire_projection_uses_the_dominant_axis;
