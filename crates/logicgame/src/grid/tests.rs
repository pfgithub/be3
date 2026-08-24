use super::*;

fn scale(value: u8) -> Scale {
    Scale::new(value).unwrap()
}

fn compiled_block() -> Uuid {
    Uuid::from_u128(0)
}

fn wire(start: (i64, i64), end: (i64, i64), scale: u8) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::new(scale).unwrap(),
    )
    .unwrap()
}

mod bounds_exclude_input_and_output_components;
mod bounds_expand_to_largest_io_scale;
mod calculates_bounds_from_components_and_scaled_wires;
mod component_orientation_canonicalizes_flips;
mod component_slots_only_connect_to_wires_at_the_same_scale;
mod component_slots_transform_with_component_orientation;
mod does_not_merge_collinear_wires_whose_endpoints_only_touch;
mod empty_grid_has_no_bounds;
mod graph_collapses_branches_and_keeps_separate_component_contacts;
mod grid_with_only_input_and_output_has_no_bounds;
mod infinite_leads_are_blocked_by_components_but_not_wires;
mod input_and_output_ids_are_unique_uuid_namespaces;
mod input_components_rotate_their_connection_slots;
mod inserting_a_component_keeps_its_id_and_ignores_repeats;
mod interior_crossings_and_mixed_scales_do_not_connect;
mod isolated_components_are_present_in_the_graph;
mod led_has_a_bottom_input_and_rejects_other_contacts;
mod led_rotation_moves_its_input_with_its_short_edge;
mod merger_combines_smaller_inputs_into_a_wide_output;
mod merges_collinear_wires_and_subtracts_partial_ranges;
mod not_gate_defines_rotated_input_and_output_slots;
mod port_connects_to_the_middle_of_a_parallel_wire;
mod preserves_a_split_at_a_perpendicular_endpoint_junction;
mod removing_a_wire_segment_preserves_touching_segments;
mod reports_snapping_with_negative_coordinates_and_overflow;
mod retains_component_overlap_errors;
mod serialized_snapshot_round_trips_and_regenerates_ids;
mod side_port_connects_to_a_wire_running_alongside_it;
mod splitter_partitions_a_wide_input_into_smaller_outputs;
mod storage_bits_toggle_independently_and_reject_out_of_range_bits;
mod storage_has_bottom_input_and_top_output;
mod storage_values_are_initialized_and_masked_to_their_scale;
mod subcomponent_ports_are_validated_and_rotate_with_the_component;
mod touching_ports_connect_directly_without_a_wire;
mod touching_ports_of_the_same_direction_do_not_connect;
mod validates_scales_wire_shapes_and_rotated_dimensions;
mod very_long_wires_remain_single_entities;
mod wire_crossing_past_a_component_terminal_is_an_intersection;
mod wire_endpoint_on_not_gate_non_slot_side_is_an_intersection;
mod wire_endpoint_on_not_gate_terminal_is_a_valid_connection;
mod wire_touching_not_gate_terminal_connects_without_overlap;
