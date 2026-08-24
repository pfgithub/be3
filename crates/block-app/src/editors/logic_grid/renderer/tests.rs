use super::*;
use logicgame::grid::{
    ComponentId, ComponentSide, ConnectionDirection, ConnectionSlot, ConnectionSlotId, InputId,
    OutputId, Point, Scale,
};

mod connection_markers_are_inward_and_stay_inside_component_bounds;
mod connection_markers_render_as_wire_value_triangles;
mod connection_stub_extends_outward_from_the_wired_port;
mod input_and_output_leads_extend_to_the_viewport_edge;
mod merger_splitter_renders_order_lines;
mod one_x_grid_emits_lines_one_world_unit_apart;
mod storage_state_renders_as_wire_value_rectangle;
mod viewport_grid_and_entities_are_layered_and_bounded;
mod wire_vertices_carry_segment_coordinates_and_value_indices;
mod wires_and_components_are_emitted_as_filled_triangles;

fn bbox(triangles: &[DrawTriangle]) -> [f32; 4] {
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for triangle in triangles {
        for [x, y] in triangle.positions {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x);
            bounds[3] = bounds[3].max(y);
        }
    }
    bounds
}
mod mirrored_merger_splitter_renders_crossing_order_lines;
