use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::grid::{
    value_mask, CircuitGraph, ComponentHash, ComponentId, ComponentKind, ConnectionDirection,
    ConnectionSlotId, GraphNode, GraphNodeId, InputId, LogicGrid, OutputId,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    Call {
        component: ComponentHash,
        inputs: Vec<Option<MemoryAddress>>,
        outputs: Vec<Vec<MemoryAddress>>,
    },
    Not {
        input: MemoryAddress,
        output: MemoryAddress,
    },
    CopyBits {
        input: MemoryAddress,
        output: MemoryAddress,
        shift: i8,
        mask: u64,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vm {
    pub memory: Vec<u64>,
    pub storage: Vec<u64>,
    pub inputs: Vec<u64>,
    pub outputs: Vec<u64>,
    pub instructions: Vec<Instruction>,
    #[serde(skip)]
    pub components: Vec<Option<Component>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Component {
    pub memory: Vec<u64>,
    pub storage: Vec<StorageId>,
    pub inputs: Vec<u64>,
    pub outputs: Vec<u64>,
    pub instructions: Vec<Instruction>,
    pub components: Vec<Option<Component>>,
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
                ComponentKind::MergerSplitter {
                    input_scale,
                    output_scale,
                } => {
                    let width = input_scale.get().max(output_scale.get());
                    let input_count = width / input_scale.get();
                    let output_count = width / output_scale.get();
                    let mut input_slots = Vec::new();
                    let mut outputs = Vec::new();
                    let mut instructions = Vec::new();
                    for slot in 0..input_count {
                        let addresses = connection_addresses(
                            &connections,
                            component.id,
                            ConnectionDirection::Input,
                            ConnectionSlotId(slot as u16),
                        );
                        if addresses.len() > 1 {
                            return Err(GenerationError::AmbiguousInput {
                                component: component.id,
                                slot: ConnectionSlotId(slot as u16),
                            });
                        }
                        if let Some(&input) = addresses.first() {
                            input_slots.push((slot, input));
                        }
                    }
                    for slot in 0..output_count {
                        outputs.extend_from_slice(connection_addresses(
                            &connections,
                            component.id,
                            ConnectionDirection::Output,
                            ConnectionSlotId((input_count + slot) as u16),
                        ));
                    }
                    if input_scale <= output_scale {
                        for &(slot, input) in &input_slots {
                            for &output in &outputs {
                                instructions.push(Instruction::CopyBits {
                                    input,
                                    output,
                                    shift: (slot * input_scale.get()) as i8,
                                    mask: value_mask(*input_scale),
                                });
                            }
                        }
                    } else if let Some(&(_, input)) = input_slots.first() {
                        for (slot, &output) in outputs.iter().enumerate() {
                            instructions.push(Instruction::CopyBits {
                                input,
                                output,
                                shift: -(slot as i64 * output_scale.get()) as i8,
                                mask: value_mask(*output_scale),
                            });
                        }
                    }
                    if !instructions.is_empty() {
                        operations.push(Operation {
                            inputs: input_slots
                                .into_iter()
                                .map(|(_, address)| address)
                                .collect(),
                            outputs,
                            instructions,
                        });
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
                ComponentKind::Subcomponent {
                    component: component_hash,
                    ports,
                    ..
                } => {
                    let input_count = ports
                        .iter()
                        .filter(|port| port.direction == ConnectionDirection::Input)
                        .map(|port| port.index + 1)
                        .max()
                        .unwrap_or(0);
                    let output_count = ports
                        .iter()
                        .filter(|port| port.direction == ConnectionDirection::Output)
                        .map(|port| port.index + 1)
                        .max()
                        .unwrap_or(0);
                    let mut inputs = vec![None; input_count];
                    let mut outputs = vec![Vec::new(); output_count];
                    for (slot, port) in ports.iter().enumerate() {
                        let addresses = connection_addresses(
                            &connections,
                            component.id,
                            port.direction,
                            ConnectionSlotId(slot as u16),
                        );
                        match port.direction {
                            ConnectionDirection::Input => {
                                if addresses.len() > 1 {
                                    return Err(GenerationError::AmbiguousInput {
                                        component: component.id,
                                        slot: ConnectionSlotId(slot as u16),
                                    });
                                }
                                inputs[port.index] = addresses.first().copied();
                            }
                            ConnectionDirection::Output => {
                                outputs[port.index].extend_from_slice(addresses);
                            }
                        }
                    }
                    operations.push(Operation {
                        inputs: inputs.iter().flatten().copied().collect(),
                        outputs: outputs.iter().flatten().copied().collect(),
                        instructions: vec![Instruction::Call {
                            component: component_hash.clone(),
                            inputs,
                            outputs,
                        }],
                    });
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
            components: Vec::new(),
        })
    }

    pub fn load_components<E>(
        &mut self,
        mut load: impl FnMut(&ComponentHash) -> Result<Vm, E>,
    ) -> Result<(), E> {
        self.components =
            load_component_instances(&self.instructions, &mut self.storage, &mut load)?;
        Ok(())
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
            Instruction::Call {
                component,
                inputs,
                outputs,
            } => {
                let instance = self
                    .components
                    .get_mut(index)
                    .and_then(Option::as_mut)
                    .unwrap_or_else(|| panic!("component {component} is not loaded"));
                instance.call(&mut self.storage, &mut self.memory, &inputs, &outputs);
            }
            Instruction::Not { input, output } => {
                self.memory[output] |= !self.memory[input];
            }
            Instruction::CopyBits {
                input,
                output,
                shift,
                mask,
            } => {
                let value = self.memory[input] & mask;
                self.memory[output] |= if shift >= 0 {
                    value << shift
                } else {
                    self.memory[input] >> -shift & mask
                };
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

impl Component {
    fn from_vm<E>(
        vm: Vm,
        root_storage: &mut Vec<u64>,
        load: &mut impl FnMut(&ComponentHash) -> Result<Vm, E>,
    ) -> Result<Self, E> {
        let storage_start = root_storage.len();
        let storage_end = storage_start + vm.storage.len();
        root_storage.extend_from_slice(&vm.storage);
        let components = load_component_instances(&vm.instructions, root_storage, load)?;
        Ok(Self {
            memory: vm.memory,
            storage: (storage_start..storage_end).collect(),
            inputs: vm.inputs,
            outputs: vm.outputs,
            instructions: vm.instructions,
            components,
        })
    }

    fn call(
        &mut self,
        root_storage: &mut [u64],
        parent_memory: &mut [u64],
        input_bindings: &[Option<MemoryAddress>],
        output_bindings: &[Vec<MemoryAddress>],
    ) {
        self.memory.fill(0);
        self.inputs.fill(0);
        for (input, binding) in self.inputs.iter_mut().zip(input_bindings) {
            if let Some(address) = binding {
                *input = parent_memory[*address];
            }
        }
        self.outputs.fill(0);

        for instruction in 0..self.instructions.len() {
            self.execute_instruction(instruction, root_storage);
        }

        for (output, bindings) in self.outputs.iter().zip(output_bindings) {
            for &address in bindings {
                parent_memory[address] |= *output;
            }
        }
    }

    fn execute_instruction(&mut self, index: usize, root_storage: &mut [u64]) {
        match self.instructions[index].clone() {
            Instruction::Call {
                component,
                inputs,
                outputs,
            } => {
                let instance = self
                    .components
                    .get_mut(index)
                    .and_then(Option::as_mut)
                    .unwrap_or_else(|| panic!("component {component} is not loaded"));
                instance.call(root_storage, &mut self.memory, &inputs, &outputs);
            }
            Instruction::Not { input, output } => {
                self.memory[output] |= !self.memory[input];
            }
            Instruction::CopyBits {
                input,
                output,
                shift,
                mask,
            } => {
                let value = self.memory[input] & mask;
                self.memory[output] |= if shift >= 0 {
                    value << shift
                } else {
                    self.memory[input] >> -shift & mask
                };
            }
            Instruction::ReadStorage { storage, output } => {
                self.memory[output] |= root_storage[self.storage[storage]];
            }
            Instruction::SaveStorage { storage, input } => {
                root_storage[self.storage[storage]] = self.memory[input];
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

fn load_component_instances<E>(
    instructions: &[Instruction],
    root_storage: &mut Vec<u64>,
    load: &mut impl FnMut(&ComponentHash) -> Result<Vm, E>,
) -> Result<Vec<Option<Component>>, E> {
    let mut components = Vec::with_capacity(instructions.len());
    for instruction in instructions {
        let instance = match instruction {
            Instruction::Call { component, .. } => {
                Some(Component::from_vm(load(component)?, root_storage, load)?)
            }
            _ => None,
        };
        components.push(instance);
    }
    Ok(components)
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
    #[should_panic(expected = "is not loaded")]
    fn unloaded_call_panics() {
        let mut vm = Vm {
            instructions: vec![Instruction::Call {
                component: ComponentHash::new("0".repeat(64)).unwrap(),
                inputs: vec![],
                outputs: vec![],
            }],
            ..Vm::default()
        };

        vm.execute();
    }

    #[test]
    fn call_clears_memory_writes_inputs_and_clears_outputs() {
        let hash = ComponentHash::new("1".repeat(64)).unwrap();
        let mut vm = Vm {
            memory: vec![5, 0],
            instructions: vec![Instruction::Call {
                component: hash,
                inputs: vec![Some(0)],
                outputs: vec![vec![1]],
            }],
            ..Vm::default()
        };
        vm.load_components(|_| {
            Ok::<_, ()>(Vm {
                memory: vec![u64::MAX, u64::MAX],
                inputs: vec![u64::MAX],
                outputs: vec![u64::MAX],
                instructions: vec![
                    Instruction::ReadInput {
                        input: InputId(0),
                        output: 0,
                        mask: u64::MAX,
                    },
                    Instruction::WriteOutput {
                        output: OutputId(0),
                        input: 0,
                        mask: u64::MAX,
                    },
                ],
                ..Vm::default()
            })
        })
        .unwrap();

        vm.execute();
        assert_eq!(vm.memory, vec![5, 5]);

        vm.memory.copy_from_slice(&[0, 0]);
        vm.execute();
        assert_eq!(vm.memory, vec![0, 0]);
        let component = vm.components[0].as_ref().unwrap();
        assert_eq!(component.memory, vec![0, 0]);
        assert_eq!(component.inputs, vec![0]);
        assert_eq!(component.outputs, vec![0]);
    }

    #[test]
    fn subcomponent_storage_persists_in_the_root_vm() {
        let hash = ComponentHash::new("2".repeat(64)).unwrap();
        let mut vm = Vm {
            memory: vec![7, 0],
            storage: vec![99],
            instructions: vec![Instruction::Call {
                component: hash,
                inputs: vec![Some(0)],
                outputs: vec![vec![1]],
            }],
            ..Vm::default()
        };
        vm.load_components(|_| {
            Ok::<_, ()>(Vm {
                memory: vec![0, 0],
                storage: vec![3],
                inputs: vec![0],
                outputs: vec![0],
                instructions: vec![
                    Instruction::ReadStorage {
                        storage: 0,
                        output: 0,
                    },
                    Instruction::WriteOutput {
                        output: OutputId(0),
                        input: 0,
                        mask: u64::MAX,
                    },
                    Instruction::ReadInput {
                        input: InputId(0),
                        output: 1,
                        mask: u64::MAX,
                    },
                    Instruction::SaveStorage {
                        storage: 0,
                        input: 1,
                    },
                ],
                ..Vm::default()
            })
        })
        .unwrap();

        assert_eq!(vm.storage, vec![99, 3]);
        assert_eq!(vm.components[0].as_ref().unwrap().storage, vec![1]);

        vm.execute();
        assert_eq!(vm.memory[1], 3);
        assert_eq!(vm.storage, vec![99, 7]);

        vm.memory.copy_from_slice(&[11, 0]);
        vm.execute();
        assert_eq!(vm.memory[1], 7);
        assert_eq!(vm.storage, vec![99, 11]);
    }

    #[test]
    fn repeated_subcomponents_have_independent_storage() {
        let hash = ComponentHash::new("3".repeat(64)).unwrap();
        let mut vm = Vm {
            memory: vec![1, 2],
            instructions: vec![
                Instruction::Call {
                    component: hash.clone(),
                    inputs: vec![Some(0)],
                    outputs: vec![],
                },
                Instruction::Call {
                    component: hash,
                    inputs: vec![Some(1)],
                    outputs: vec![],
                },
            ],
            ..Vm::default()
        };
        vm.load_components(|_| {
            Ok::<_, ()>(Vm {
                memory: vec![0],
                storage: vec![0],
                inputs: vec![0],
                instructions: vec![
                    Instruction::ReadInput {
                        input: InputId(0),
                        output: 0,
                        mask: u64::MAX,
                    },
                    Instruction::SaveStorage {
                        storage: 0,
                        input: 0,
                    },
                ],
                ..Vm::default()
            })
        })
        .unwrap();

        vm.execute();

        assert_eq!(vm.storage, vec![1, 2]);
        assert_eq!(vm.components[0].as_ref().unwrap().storage, vec![0]);
        assert_eq!(vm.components[1].as_ref().unwrap().storage, vec![1]);
    }

    #[test]
    fn nested_subcomponent_storage_is_owned_by_the_root_vm() {
        let middle_hash = ComponentHash::new("4".repeat(64)).unwrap();
        let leaf_hash = ComponentHash::new("5".repeat(64)).unwrap();
        let mut vm = Vm {
            memory: vec![13],
            instructions: vec![Instruction::Call {
                component: middle_hash.clone(),
                inputs: vec![Some(0)],
                outputs: vec![],
            }],
            ..Vm::default()
        };
        vm.load_components(|hash| {
            if hash == &middle_hash {
                Ok::<_, ()>(Vm {
                    memory: vec![0],
                    inputs: vec![0],
                    instructions: vec![
                        Instruction::ReadInput {
                            input: InputId(0),
                            output: 0,
                            mask: u64::MAX,
                        },
                        Instruction::Call {
                            component: leaf_hash.clone(),
                            inputs: vec![Some(0)],
                            outputs: vec![],
                        },
                    ],
                    ..Vm::default()
                })
            } else {
                Ok(Vm {
                    memory: vec![0],
                    storage: vec![8],
                    inputs: vec![0],
                    instructions: vec![
                        Instruction::ReadInput {
                            input: InputId(0),
                            output: 0,
                            mask: u64::MAX,
                        },
                        Instruction::SaveStorage {
                            storage: 0,
                            input: 0,
                        },
                    ],
                    ..Vm::default()
                })
            }
        })
        .unwrap();

        vm.execute();

        assert_eq!(vm.storage, vec![13]);
        let middle = vm.components[0].as_ref().unwrap();
        assert!(middle.storage.is_empty());
        assert_eq!(middle.components[1].as_ref().unwrap().storage, vec![0]);
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
    fn inputs_and_outputs_execute_in_dependency_order() {
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

    #[test]
    fn splitter_extracts_low_to_high_chunks_in_slot_order() {
        let mut grid = LogicGrid::new();
        let splitter = grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::MergerSplitter {
                input_scale: Scale::new(16).unwrap(),
                output_scale: Scale::new(4).unwrap(),
            },
        );
        let graph = graph(
            5,
            &[
                (splitter, ConnectionDirection::Input, 0, 0),
                (splitter, ConnectionDirection::Output, 1, 1),
                (splitter, ConnectionDirection::Output, 2, 2),
                (splitter, ConnectionDirection::Output, 3, 3),
                (splitter, ConnectionDirection::Output, 4, 4),
            ],
        );
        let mut vm = Vm::from_graph(&grid, &graph).unwrap();
        vm.memory[0] = 0xabcd;

        vm.execute();

        assert_eq!(vm.memory, vec![0xabcd, 0xd, 0xc, 0xb, 0xa]);
    }

    #[test]
    fn merger_packs_low_to_high_chunks_in_slot_order() {
        let mut grid = LogicGrid::new();
        let merger = grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::MergerSplitter {
                input_scale: Scale::new(4).unwrap(),
                output_scale: Scale::new(16).unwrap(),
            },
        );
        let graph = graph(
            5,
            &[
                (merger, ConnectionDirection::Input, 0, 0),
                (merger, ConnectionDirection::Input, 1, 1),
                (merger, ConnectionDirection::Input, 2, 2),
                (merger, ConnectionDirection::Input, 3, 3),
                (merger, ConnectionDirection::Output, 4, 4),
            ],
        );
        let mut vm = Vm::from_graph(&grid, &graph).unwrap();
        vm.memory[..4].copy_from_slice(&[0xd, 0xc, 0xb, 0xa]);

        vm.execute();

        assert_eq!(vm.memory[4], 0xabcd);
    }

    #[test]
    fn merger_preserves_bit_positions_when_an_input_is_unconnected() {
        let mut grid = LogicGrid::new();
        let merger = grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::MergerSplitter {
                input_scale: Scale::new(4).unwrap(),
                output_scale: Scale::new(16).unwrap(),
            },
        );
        let graph = graph(
            4,
            &[
                (merger, ConnectionDirection::Input, 1, 0),
                (merger, ConnectionDirection::Input, 2, 1),
                (merger, ConnectionDirection::Input, 3, 2),
                (merger, ConnectionDirection::Output, 4, 3),
            ],
        );
        let mut vm = Vm::from_graph(&grid, &graph).unwrap();
        vm.memory[..3].copy_from_slice(&[0xc, 0xb, 0xa]);

        vm.execute();

        assert_eq!(vm.memory[3], 0xabc0);
    }

    #[test]
    fn subcomponents_compile_sparse_bindings_and_output_fanout() {
        let mut grid = LogicGrid::new();
        let component_hash = ComponentHash::new("a".repeat(64)).unwrap();
        let subcomponent = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::subcomponent(
                component_hash.clone(),
                crate::grid::Size::new(4, 4),
                vec![
                    crate::grid::ComponentPort::input(1, Scale::ONE, ComponentSide::Left, 0, 1),
                    crate::grid::ComponentPort::output(2, Scale::ONE, ComponentSide::Right, 0, 1),
                ],
            )
            .unwrap(),
        );
        let graph = CircuitGraph {
            nodes: vec![
                GraphNode::WireNet { wires: Vec::new() },
                GraphNode::WireNet { wires: Vec::new() },
                GraphNode::WireNet { wires: Vec::new() },
                GraphNode::Connection {
                    component: subcomponent,
                    slot: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Left,
                    start: 0,
                    end: 1,
                    scale: Scale::ONE,
                },
                GraphNode::Connection {
                    component: subcomponent,
                    slot: ConnectionSlotId(1),
                    direction: ConnectionDirection::Output,
                    side: ComponentSide::Right,
                    start: 0,
                    end: 1,
                    scale: Scale::ONE,
                },
            ],
            edges: vec![
                GraphEdge {
                    first: GraphNodeId(0),
                    second: GraphNodeId(3),
                },
                GraphEdge {
                    first: GraphNodeId(1),
                    second: GraphNodeId(4),
                },
                GraphEdge {
                    first: GraphNodeId(2),
                    second: GraphNodeId(4),
                },
            ],
        };

        let vm = Vm::from_graph(&grid, &graph).unwrap();

        assert_eq!(
            vm.instructions,
            vec![Instruction::Call {
                component: component_hash,
                inputs: vec![None, Some(0)],
                outputs: vec![vec![], vec![], vec![1, 2]],
            }]
        );
    }
}
