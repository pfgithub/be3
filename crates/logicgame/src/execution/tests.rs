use uuid::Uuid;

use super::*;
use crate::grid::{
    ComponentSide, ComponentSubgraph, GraphEdge, InputId, OutputId, Point, Rotation, Scale, Size,
};

fn graph(
    net_count: usize,
    connections: &[(ComponentId, ConnectionDirection, u16, usize)],
) -> CircuitGraph {
    let mut nodes: Vec<_> = (0..net_count)
        .map(|_| GraphNode::WireNet { wires: Vec::new() })
        .collect();
    let mut edges = Vec::new();
    for &(component, direction, slot, net) in connections {
        let connection = GraphNodeId(nodes.len());
        nodes.push(GraphNode::Connection {
            component,
            slot: ConnectionSlotId(slot),
            direction,
            side: ComponentSide::Top,
            start: 0,
            end: 1,
            scale: Scale::ONE,
        });
        edges.push(GraphEdge {
            first: GraphNodeId(net),
            second: connection,
        });
    }
    CircuitGraph { nodes, edges }
}

fn add_not(grid: &mut LogicGrid) -> ComponentId {
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    )
}

fn add_storage(grid: &mut LogicGrid, value: u64) -> ComponentId {
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Storage {
            scale: Scale::ONE,
            value,
        },
    )
}

fn component(
    memory_size: usize,
    storage_init: Vec<u64>,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
    instructions: Vec<Instruction>,
) -> Rc<Component> {
    component_with_children(
        memory_size,
        storage_init,
        inputs,
        outputs,
        Vec::new(),
        instructions,
    )
}

fn component_with_children(
    memory_size: usize,
    storage_init: Vec<u64>,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
    components: Vec<Rc<Component>>,
    instructions: Vec<Instruction>,
) -> Rc<Component> {
    let subgraph_instructions = instructions.clone();
    let subgraph_inputs = (0..inputs.len()).collect();
    let subgraph_outputs = (0..outputs.len()).collect();
    Rc::new(Component {
        memory_size,
        storage_init,
        inputs,
        outputs,
        components,
        instructions,
        subgraphs: vec![ComponentExecutionSubgraph {
            inputs: subgraph_inputs,
            outputs: subgraph_outputs,
            instructions: subgraph_instructions,
        }],
        source: None,
    })
}

fn component_with_subgraphs(
    memory_size: usize,
    storage_init: Vec<u64>,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
    instructions: Vec<Instruction>,
    subgraphs: Vec<ComponentExecutionSubgraph>,
) -> Rc<Component> {
    Rc::new(Component {
        memory_size,
        storage_init,
        inputs,
        outputs,
        components: Vec::new(),
        instructions,
        subgraphs,
        source: None,
    })
}

fn vm_with_root(root: Rc<Component>) -> Vm {
    let mut vm = Vm::from_unlinked_component(root);
    vm.begin_tick();
    vm
}

mod call_clears_memory_writes_inputs_and_outputs_through_bindings;
mod explicit_uuid_ports_compile_to_dense_memory_bindings;
mod graph_generates_in_dependency_order;
mod inputs_and_outputs_compile_to_dense_memory_bindings;
mod instructions_can_execute_one_at_a_time;
mod nested_subcomponent_storage_is_owned_by_the_root_vm;
mod not_reads_and_writes_memory;
mod not_with_unconnected_input_reads_zero;
mod real_cycle_across_subcomponent_subgraphs_is_rejected;
mod repeated_subcomponents_share_code_and_have_independent_storage;
mod separate_subcomponent_instances_keep_independent_storage;
mod split_calls_for_one_subcomponent_instance_share_storage;
mod split_subcomponent_outputs_can_feed_later_inputs;
mod splitter_extracts_low_to_high_chunks_in_slot_order;
mod storage_can_be_saved_and_read;
mod subcomponents_compile_sparse_bindings;
mod total_instruction_count_includes_called_components_per_call;
mod total_latency_counts_dependent_gate_chain;
mod total_latency_counts_save_storage_as_terminal_work;
mod total_latency_includes_subcomponent_latency;
mod total_latency_uses_parallel_branch_depth;
mod unloaded_call_panics;
