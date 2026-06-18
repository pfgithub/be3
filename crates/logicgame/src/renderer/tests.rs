use super::*;
use logicgame::grid::{
    ComponentId, ComponentSide, ConnectionDirection, ConnectionSlot, ConnectionSlotId, InputId,
    OutputId, Point, Scale,
};

mod connection_markers_are_inward_and_stay_inside_component_bounds;
mod connection_stub_extends_outward_from_the_wired_port;
mod input_and_output_leads_extend_to_the_viewport_edge;
mod one_x_grid_emits_lines_one_world_unit_apart;
mod viewport_grid_and_entities_are_layered_and_bounded;
mod wire_vertices_carry_segment_coordinates_and_value_indices;
mod wires_and_components_are_emitted_as_filled_triangles;
