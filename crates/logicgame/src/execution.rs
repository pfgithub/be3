use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::grid::{
    value_mask, CircuitGraph, ComponentId, ComponentKind, ConnectionDirection, ConnectionSlotId,
    GraphNode, GraphNodeId, InputId, LogicGrid, OutputId,
};

pub type MemoryAddress = usize;
pub type StorageId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationError {
    InvalidGraph,
    UnsupportedComponent(ComponentId),
    AmbiguousInput {
        component: ComponentId,
        slot: ConnectionSlotId,
    },
    Cycle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    Call {
        component: Uuid,
        inputs: Vec<MemoryAddress>,
        outputs: Vec<MemoryAddress>,
    },
    Not {
        input: MemoryAddress,
        output: MemoryAddress,
    },
    ReadStorage {
        storage: StorageId,
        output: MemoryAddress,
    },
    SaveStorage {
        storage: StorageId,
        input: MemoryAddress,
    },
    ReadInput {
        input: InputId,
        output: MemoryAddress,
        mask: u64,
    },
    WriteOutput {
        output: OutputId,
        input: MemoryAddress,
        mask: u64,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vm {
    pub memory: Vec<u64>,
    pub storage: Vec<u64>,
    pub inputs: Vec<u64>,
    pub outputs: Vec<u64>,
    pub instructions: Vec<Instruction>,
}

impl Vm {
    pub fn from_graph(grid: &LogicGrid, graph: &CircuitGraph) -> Result<Self, GenerationError> {
        let memory_addresses: BTreeMap<_, _> = graph
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(node, value)| {
                matches!(value, GraphNode::WireNet { .. }).then_some(GraphNodeId(node))
            })
            .enumerate()
            .map(|(address, node)| (node, address))
            .collect();

        let mut connections = BTreeMap::<
            (ComponentId, ConnectionDirection, ConnectionSlotId),
            Vec<MemoryAddress>,
        >::new();
        for (node_index, node) in graph.nodes.iter().enumerate() {
            let GraphNode::Connection {
                component,
                slot,
                direction,
                ..
            } = node
            else {
                continue;
            };

            let mut addresses = Vec::new();
            for edge in &graph.edges {
                let adjacent = if edge.first == GraphNodeId(node_index) {
                    edge.second
                } else if edge.second == GraphNodeId(node_index) {
                    edge.first
                } else {
                    continue;
                };
                if adjacent.0 >= graph.nodes.len() {
                    return Err(GenerationError::InvalidGraph);
                }
                if let Some(address) = memory_addresses.get(&adjacent) {
                    addresses.push(*address);
                }
            }
            addresses.sort_unstable();
            addresses.dedup();
            connections.insert((*component, *direction, *slot), addresses);
        }

        let mut storage_ids = BTreeMap::new();
        let mut storage = Vec::new();
        let mut input_count = 0;
        let mut output_count = 0;
        for component in grid.components() {
            match &component.kind {
                ComponentKind::Storage { value, .. } => {
                    storage_ids.insert(component.id, storage.len());
                    storage.push(*value);
                }
                ComponentKind::Input { id, .. } => input_count = input_count.max(id.0 + 1),
                ComponentKind::Output { id, .. } => output_count = output_count.max(id.0 + 1),
                _ => {}
            }
        }

        let mut operations = Vec::new();
        for component in grid.components() {
            match &component.kind {
                ComponentKind::Not { .. } => {
                    let inputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Input,
                        ConnectionSlotId(0),
                    );
                    let outputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Output,
                        ConnectionSlotId(1),
                    );
                    if inputs.len() > 1 {
                        return Err(GenerationError::AmbiguousInput {
                            component: component.id,
                            slot: ConnectionSlotId(0),
                        });
                    }
                    if let Some(&input) = inputs.first() {
                        if !outputs.is_empty() {
                            operations.push(Operation {
                                inputs: vec![input],
                                outputs: outputs.to_vec(),
                                instructions: outputs
                                    .iter()
                                    .map(|&output| Instruction::Not { input, output })
                                    .collect(),
                            });
                        }
                    }
                }
                ComponentKind::Storage { .. } => {
                    let storage = storage_ids[&component.id];
                    let outputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Output,
                        ConnectionSlotId(1),
                    );
                    if !outputs.is_empty() {
                        operations.push(Operation {
                            inputs: Vec::new(),
                            outputs: outputs.to_vec(),
                            instructions: outputs
                                .iter()
                                .map(|&output| Instruction::ReadStorage { storage, output })
                                .collect(),
                        });
                    }

                    let inputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Input,
                        ConnectionSlotId(0),
                    );
                    if inputs.len() > 1 {
                        return Err(GenerationError::AmbiguousInput {
                            component: component.id,
                            slot: ConnectionSlotId(0),
                        });
                    }
                    if let Some(&input) = inputs.first() {
                        operations.push(Operation {
                            inputs: vec![input],
                            outputs: Vec::new(),
                            instructions: vec![Instruction::SaveStorage { storage, input }],
                        });
                    }
                }
                ComponentKind::Input { scale, id } => {
                    let outputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Output,
                        ConnectionSlotId(0),
                    );
                    if !outputs.is_empty() {
                        operations.push(Operation {
                            inputs: Vec::new(),
                            outputs: outputs.to_vec(),
                            instructions: outputs
                                .iter()
                                .map(|&output| Instruction::ReadInput {
                                    input: *id,
                                    output,
                                    mask: value_mask(*scale),
                                })
                                .collect(),
                        });
                    }
                }
                ComponentKind::Output { scale, id } => {
                    let inputs = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Input,
                        ConnectionSlotId(0),
                    );
                    if inputs.len() > 1 {
                        return Err(GenerationError::AmbiguousInput {
                            component: component.id,
                            slot: ConnectionSlotId(0),
                        });
                    }
                    if let Some(&input) = inputs.first() {
                        operations.push(Operation {
                            inputs: vec![input],
                            outputs: Vec::new(),
                            instructions: vec![Instruction::WriteOutput {
                                output: *id,
                                input,
                                mask: value_mask(*scale),
                            }],
                        });
                    }
                }
                ComponentKind::Led => {}
                ComponentKind::Subcomponent { .. } => {
                    return Err(GenerationError::UnsupportedComponent(component.id));
                }
            }
        }

        let mut writers = vec![Vec::new(); memory_addresses.len()];
        for (operation_index, operation) in operations.iter().enumerate() {
            for &output in &operation.outputs {
                writers[output].push(operation_index);
            }
        }

        let mut dependencies = vec![BTreeSet::new(); operations.len()];
        let mut dependents = vec![BTreeSet::new(); operations.len()];
        for (consumer, operation) in operations.iter().enumerate() {
            for &input in &operation.inputs {
                for &writer in &writers[input] {
                    dependencies[consumer].insert(writer);
                    dependents[writer].insert(consumer);
                }
            }
        }

        let mut ready: BTreeSet<_> = dependencies
            .iter()
            .enumerate()
            .filter_map(|(index, dependencies)| dependencies.is_empty().then_some(index))
            .collect();
        let mut order = Vec::with_capacity(operations.len());
        while let Some(operation) = ready.pop_first() {
            order.push(operation);
            for &dependent in &dependents[operation] {
                dependencies[dependent].remove(&operation);
                if dependencies[dependent].is_empty() {
                    ready.insert(dependent);
                }
            }
        }
        if order.len() != operations.len() {
            return Err(GenerationError::Cycle);
        }

        let instructions = order
            .into_iter()
            .flat_map(|operation| operations[operation].instructions.clone())
            .collect();
        Ok(Self {
            memory: vec![0; memory_addresses.len()],
            storage,
            inputs: vec![0; input_count],
            outputs: vec![0; output_count],
            instructions,
        })
    }

    pub fn begin_tick(&mut self) {
        self.memory.fill(0);
        self.outputs.fill(0);
    }

    pub fn execute(&mut self) {
        for instruction in 0..self.instructions.len() {
            self.execute_instruction(instruction);
        }
    }

    pub fn execute_instruction(&mut self, index: usize) {
        match self.instructions[index].clone() {
            Instruction::Call { component, .. } => {
                panic!("calling component {component} is not implemented")
            }
            Instruction::Not { input, output } => {
                self.memory[output] |= !self.memory[input];
            }
            Instruction::ReadStorage { storage, output } => {
                self.memory[output] |= self.storage[storage];
            }
            Instruction::SaveStorage { storage, input } => {
                self.storage[storage] = self.memory[input];
            }
            Instruction::ReadInput {
                input,
                output,
                mask,
            } => {
                self.memory[output] |= self.inputs[input.0] & mask;
            }
            Instruction::WriteOutput {
                output,
                input,
                mask,
            } => {
                self.outputs[output.0] = self.memory[input] & mask;
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Operation {
    inputs: Vec<MemoryAddress>,
    outputs: Vec<MemoryAddress>,
    instructions: Vec<Instruction>,
}

fn connection_addresses(
    connections: &BTreeMap<
        (ComponentId, ConnectionDirection, ConnectionSlotId),
        Vec<MemoryAddress>,
    >,
    component: ComponentId,
    direction: ConnectionDirection,
    slot: ConnectionSlotId,
) -> &[MemoryAddress] {
    connections
        .get(&(component, direction, slot))
        .map(Vec::as_slice)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{ComponentSide, GraphEdge, InputId, OutputId, Point, Rotation, Scale};

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

    #[test]
    fn not_reads_and_writes_memory() {
        let mut vm = Vm {
            memory: vec![0x00ff, 0],
            instructions: vec![Instruction::Not {
                input: 0,
                output: 1,
            }],
            ..Vm::default()
        };

        vm.execute();

        assert_eq!(vm.memory, vec![0x00ff, !0x00ff]);
    }

    #[test]
    fn storage_can_be_saved_and_read() {
        let mut vm = Vm {
            memory: vec![42, 0],
            storage: vec![0],
            instructions: vec![
                Instruction::SaveStorage {
                    storage: 0,
                    input: 0,
                },
                Instruction::ReadStorage {
                    storage: 0,
                    output: 1,
                },
            ],
            ..Vm::default()
        };

        vm.execute();

        assert_eq!(vm.storage, vec![42]);
        assert_eq!(vm.memory, vec![42, 42]);
    }

    #[test]
    fn writes_are_combined_with_bitwise_or() {
        let mut vm = Vm {
            memory: vec![0, 1],
            storage: vec![2, 4],
            instructions: vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::ReadStorage {
                    storage: 1,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
            ],
            ..Vm::default()
        };

        vm.execute();

        assert_eq!(vm.memory, vec![6, 1 | !6]);
    }

    #[test]
    fn instructions_execute_in_order() {
        let mut vm = Vm {
            memory: vec![0, 0],
            storage: vec![7],
            instructions: vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::SaveStorage {
                    storage: 0,
                    input: 1,
                },
            ],
            ..Vm::default()
        };

        vm.execute();

        assert_eq!(vm.storage[0], !7);
    }

    #[test]
    fn instructions_can_execute_one_at_a_time() {
        let mut vm = Vm {
            memory: vec![0, 0],
            storage: vec![7],
            instructions: vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
            ],
            ..Vm::default()
        };

        vm.execute_instruction(0);
        assert_eq!(vm.memory, vec![7, 0]);

        vm.execute_instruction(1);
        assert_eq!(vm.memory, vec![7, !7]);
    }

    #[test]
    #[should_panic(expected = "is not implemented")]
    fn call_is_not_implemented() {
        let mut vm = Vm {
            instructions: vec![Instruction::Call {
                component: Uuid::nil(),
                inputs: vec![],
                outputs: vec![],
            }],
            ..Vm::default()
        };

        vm.execute();
    }

    #[test]
    fn graph_generates_in_dependency_order() {
        let mut grid = LogicGrid::new();
        let source = add_storage(&mut grid, 7);
        let first_not = add_not(&mut grid);
        let second_not = add_not(&mut grid);
        let destination = add_storage(&mut grid, 0);
        let graph = graph(
            3,
            &[
                (source, ConnectionDirection::Output, 1, 0),
                (first_not, ConnectionDirection::Input, 0, 0),
                (first_not, ConnectionDirection::Output, 1, 1),
                (second_not, ConnectionDirection::Input, 0, 1),
                (second_not, ConnectionDirection::Output, 1, 2),
                (destination, ConnectionDirection::Input, 0, 2),
            ],
        );

        let vm = Vm::from_graph(&grid, &graph).unwrap();

        assert_eq!(vm.memory, vec![0; 3]);
        assert_eq!(vm.storage, vec![1, 0]);
        assert_eq!(
            vm.instructions,
            vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::Not {
                    input: 1,
                    output: 2,
                },
                Instruction::SaveStorage {
                    storage: 1,
                    input: 2,
                },
            ]
        );
    }

    #[test]
    fn all_writers_run_before_a_wire_is_read() {
        let mut grid = LogicGrid::new();
        let first = add_storage(&mut grid, 1);
        let second = add_storage(&mut grid, 0);
        let not = add_not(&mut grid);
        let graph = graph(
            2,
            &[
                (first, ConnectionDirection::Output, 1, 0),
                (second, ConnectionDirection::Output, 1, 0),
                (not, ConnectionDirection::Input, 0, 0),
                (not, ConnectionDirection::Output, 1, 1),
            ],
        );

        let mut vm = Vm::from_graph(&grid, &graph).unwrap();

        assert!(matches!(
            vm.instructions.as_slice(),
            [
                Instruction::ReadStorage { storage: 0, .. },
                Instruction::ReadStorage { storage: 1, .. },
                Instruction::Not { .. }
            ]
        ));
        vm.execute();
        assert_eq!(vm.memory[1], !1);
    }

    #[test]
    fn combinational_cycles_are_rejected() {
        let mut grid = LogicGrid::new();
        let first = add_not(&mut grid);
        let second = add_not(&mut grid);
        let graph = graph(
            2,
            &[
                (first, ConnectionDirection::Input, 0, 1),
                (first, ConnectionDirection::Output, 1, 0),
                (second, ConnectionDirection::Input, 0, 0),
                (second, ConnectionDirection::Output, 1, 1),
            ],
        );

        assert_eq!(Vm::from_graph(&grid, &graph), Err(GenerationError::Cycle));
    }

    #[test]
    fn storage_breaks_feedback_cycles() {
        let mut grid = LogicGrid::new();
        let storage = add_storage(&mut grid, 1);
        let not = add_not(&mut grid);
        let graph = graph(
            2,
            &[
                (storage, ConnectionDirection::Output, 1, 0),
                (not, ConnectionDirection::Input, 0, 0),
                (not, ConnectionDirection::Output, 1, 1),
                (storage, ConnectionDirection::Input, 0, 1),
            ],
        );

        let mut vm = Vm::from_graph(&grid, &graph).unwrap();
        vm.execute();

        assert_eq!(vm.storage, vec![!1]);
    }

    #[test]
    fn boundary_inputs_and_outputs_execute_in_dependency_order() {
        let mut grid = LogicGrid::new();
        let removed_input = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(99),
            },
        );
        grid.remove_component(removed_input);
        let input = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::new(4).unwrap(),
                id: InputId(99),
            },
        );
        let removed_output = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId(99),
            },
        );
        grid.remove_component(removed_output);
        let output = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::new(4).unwrap(),
                id: OutputId(99),
            },
        );
        let graph = graph(
            1,
            &[
                (input, ConnectionDirection::Output, 0, 0),
                (output, ConnectionDirection::Input, 0, 0),
            ],
        );

        let mut vm = Vm::from_graph(&grid, &graph).unwrap();
        assert_eq!(vm.inputs, vec![0, 0]);
        assert_eq!(vm.outputs, vec![0, 0]);
        assert!(matches!(
            vm.instructions.as_slice(),
            [
                Instruction::ReadInput {
                    input: InputId(1),
                    output: 0,
                    ..
                },
                Instruction::WriteOutput {
                    output: OutputId(1),
                    input: 0,
                    ..
                }
            ]
        ));

        vm.inputs[1] = 0xff;
        vm.begin_tick();
        vm.execute();
        assert_eq!(vm.memory, vec![0x0f]);
        assert_eq!(vm.outputs, vec![0, 0x0f]);

        vm.memory[0] = u64::MAX;
        vm.outputs[1] = u64::MAX;
        vm.begin_tick();
        assert_eq!(vm.inputs[1], 0xff);
        assert_eq!(vm.memory, vec![0]);
        assert_eq!(vm.outputs, vec![0, 0]);
    }
}
