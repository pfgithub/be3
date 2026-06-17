use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

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
        component: usize,
        #[serde(default)]
        instance: usize,
        #[serde(default)]
        subgraph: usize,
        #[serde(default, skip_serializing)]
        storage_offset: StorageId,
        inputs: Vec<Option<MemoryAddress>>,
        outputs: Vec<Option<MemoryAddress>>,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedComponent {
    pub inputs: Vec<MemoryAddress>,
    pub outputs: Vec<MemoryAddress>,
    pub components: Vec<ComponentHash>,
    pub instructions: Vec<Instruction>,
    #[serde(default)]
    pub subgraphs: Vec<UnlinkedSubgraph>,
    pub memory_size: usize,
    pub storage_init: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedSubgraph {
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub inputs: Vec<MemoryAddress>,
    pub outputs: Vec<MemoryAddress>,
    pub components: Vec<Rc<Component>>,
    pub instructions: Vec<Instruction>,
    pub subgraphs: Vec<ComponentExecutionSubgraph>,
    pub memory_size: usize,
    pub storage_init: Vec<u64>,
    pub source_hash: Option<ComponentHash>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentExecutionSubgraph {
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pc {
    pub instruction_index: usize,
    pub subgraph: Option<usize>,
    pub memory_offset: usize,
    pub storage_offset: usize,
    pub component: Rc<Component>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vm {
    pub root_component: Rc<Component>,
    pub pc: Pc,
    pub memory_stack: Vec<u64>,
    pub storage: Vec<u64>,
    pub returns: Vec<Pc>,
}

impl Default for UnlinkedComponent {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            components: Vec::new(),
            instructions: Vec::new(),
            subgraphs: Vec::new(),
            memory_size: 0,
            storage_init: Vec::new(),
        }
    }
}

impl Default for Component {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            components: Vec::new(),
            instructions: Vec::new(),
            subgraphs: Vec::new(),
            memory_size: 0,
            storage_init: Vec::new(),
            source_hash: None,
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        let root_component = Rc::new(Component::default());
        let pc = Pc {
            instruction_index: 0,
            subgraph: None,
            memory_offset: 0,
            storage_offset: 0,
            component: Rc::clone(&root_component),
        };
        Self {
            root_component,
            pc,
            memory_stack: Vec::new(),
            storage: Vec::new(),
            returns: Vec::new(),
        }
    }
}

impl UnlinkedComponent {
    pub fn from_graph(grid: &LogicGrid, graph: &CircuitGraph) -> Result<Self, GenerationError> {
        let mut components = Vec::new();
        let mut component_indices = BTreeMap::new();
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
        let zero_address = memory_addresses.len();
        let mut uses_zero_address = false;

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
        let mut storage_init = Vec::new();
        let input_indices = dense_input_indices(grid);
        let output_indices = dense_output_indices(grid);
        let mut inputs = vec![0; input_indices.len()];
        let mut outputs = vec![0; output_indices.len()];
        for component in grid.components() {
            match &component.kind {
                ComponentKind::Storage { value, .. } => {
                    storage_ids.insert(component.id, storage_init.len());
                    storage_init.push(*value);
                }
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
                    if !outputs.is_empty() {
                        let input = match inputs.first() {
                            Some(&input) => input,
                            None => {
                                uses_zero_address = true;
                                zero_address
                            }
                        };
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
                ComponentKind::Input { id, .. } => {
                    let addresses = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Output,
                        ConnectionSlotId(0),
                    );
                    if addresses.len() > 1 {
                        return Err(GenerationError::AmbiguousInput {
                            component: component.id,
                            slot: ConnectionSlotId(0),
                        });
                    }
                    if let (Some(&index), Some(&address)) =
                        (input_indices.get(id), addresses.first())
                    {
                        inputs[index] = address;
                    }
                }
                ComponentKind::Output { id, .. } => {
                    let addresses = connection_addresses(
                        &connections,
                        component.id,
                        ConnectionDirection::Input,
                        ConnectionSlotId(0),
                    );
                    if addresses.len() > 1 {
                        return Err(GenerationError::AmbiguousInput {
                            component: component.id,
                            slot: ConnectionSlotId(0),
                        });
                    }
                    if let (Some(&index), Some(&address)) =
                        (output_indices.get(id), addresses.first())
                    {
                        outputs[index] = address;
                    }
                }
                ComponentKind::Led => {}
                ComponentKind::Subcomponent {
                    component: component_hash,
                    ports,
                    subgraphs,
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
                    let mut outputs = vec![None; output_count];
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
                                if addresses.len() > 1 {
                                    return Err(GenerationError::AmbiguousInput {
                                        component: component.id,
                                        slot: ConnectionSlotId(slot as u16),
                                    });
                                }
                                outputs[port.index] = addresses.first().copied();
                            }
                        }
                    }
                    let child_component = *component_indices
                        .entry(component_hash.clone())
                        .or_insert_with(|| {
                            let index = components.len();
                            components.push(component_hash.clone());
                            index
                        });
                    let instance = component.id.0 as usize;
                    for (subgraph_index, subgraph) in subgraphs.iter().enumerate() {
                        let call_inputs = subgraph
                            .inputs
                            .iter()
                            .filter_map(|&input| inputs.get(input).copied().flatten())
                            .collect::<Vec<_>>();
                        let call_outputs = subgraph
                            .outputs
                            .iter()
                            .filter_map(|&output| outputs.get(output).copied().flatten())
                            .collect::<Vec<_>>();
                        let has_connected_output = !call_outputs.is_empty();
                        let has_connected_input = !call_inputs.is_empty();
                        if has_connected_output
                            || (subgraph.outputs.is_empty() && has_connected_input)
                        {
                            operations.push(Operation {
                                inputs: call_inputs,
                                outputs: call_outputs,
                                instructions: vec![Instruction::Call {
                                    component: child_component,
                                    instance,
                                    subgraph: subgraph_index,
                                    storage_offset: 0,
                                    inputs: inputs.clone(),
                                    outputs: outputs.clone(),
                                }],
                            });
                        }
                    }
                }
            }
        }

        let memory_size = memory_addresses.len() + usize::from(uses_zero_address);
        let mut writers = vec![Vec::new(); memory_size];
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

        let subgraphs = build_subgraphs(&operations, &order, &inputs, &outputs);
        let instructions = order
            .iter()
            .copied()
            .flat_map(|operation| operations[operation].instructions.clone())
            .collect();
        Ok(Self {
            inputs,
            outputs,
            components,
            instructions,
            subgraphs,
            memory_size,
            storage_init,
        })
    }

    pub fn link<E>(
        &self,
        mut load: impl FnMut(&ComponentHash) -> Result<Rc<Component>, E>,
    ) -> Result<Rc<Component>, E> {
        link_unlinked_component(self, None, &mut load)
    }

    pub fn link_with_hash<E>(
        &self,
        hash: ComponentHash,
        mut load: impl FnMut(&ComponentHash) -> Result<Rc<Component>, E>,
    ) -> Result<Rc<Component>, E> {
        link_unlinked_component(self, Some(hash), &mut load)
    }
}

impl Vm {
    pub fn from_graph(grid: &LogicGrid, graph: &CircuitGraph) -> Result<Self, GenerationError> {
        Ok(Self::from_unlinked_component(
            UnlinkedComponent::from_graph(grid, graph)?.link(|hash| {
                Ok::<_, GenerationError>(Rc::new(Component::unresolved(hash.clone())))
            })?,
        ))
    }

    pub fn from_unlinked_component(component: Rc<Component>) -> Self {
        let pc = Pc {
            instruction_index: 0,
            subgraph: None,
            memory_offset: 0,
            storage_offset: 0,
            component: Rc::clone(&component),
        };
        Self {
            storage: component.storage_init.clone(),
            root_component: component,
            pc,
            memory_stack: Vec::new(),
            returns: Vec::new(),
        }
    }

    pub fn load_components<E>(
        &mut self,
        mut load: impl FnMut(&ComponentHash) -> Result<Rc<Component>, E>,
    ) -> Result<(), E> {
        let unlinked = self.root_component.to_unlinked();
        self.root_component = unlinked.link(&mut load)?;
        self.storage = self.root_component.storage_init.clone();
        self.pc = Pc {
            instruction_index: 0,
            subgraph: None,
            memory_offset: 0,
            storage_offset: 0,
            component: Rc::clone(&self.root_component),
        };
        self.memory_stack.clear();
        self.returns.clear();
        Ok(())
    }

    pub fn begin_tick(&mut self) {
        self.memory_stack.clear();
        self.memory_stack.resize(self.root_component.memory_size, 0);
        self.returns.clear();
        self.pc = Pc {
            instruction_index: 0,
            subgraph: None,
            memory_offset: 0,
            storage_offset: 0,
            component: Rc::clone(&self.root_component),
        };
    }

    pub fn is_tick_complete(&self) -> bool {
        self.returns.is_empty()
            && self.pc.memory_offset == 0
            && self.pc.instruction_index >= self.root_component.instructions.len()
    }

    pub fn execute(&mut self) {
        if self.memory_stack.len() != self.root_component.memory_size {
            self.begin_tick();
        }
        while !self.is_tick_complete() {
            self.execute_instruction();
        }
    }

    pub fn execute_instruction(&mut self) {
        if self.memory_stack.len() < self.pc.memory_offset + self.pc.component.memory_size {
            self.memory_stack
                .resize(self.pc.memory_offset + self.pc.component.memory_size, 0);
        }
        if self.pc.instruction_index >= self.pc.instructions().len() {
            self.return_from_component();
            return;
        }

        match self.pc.instructions()[self.pc.instruction_index].clone() {
            Instruction::Call {
                component,
                subgraph,
                storage_offset,
                inputs,
                ..
            } => {
                let child = Rc::clone(&self.pc.component.components[component]);
                if let Some(hash) = child.unresolved_hash() {
                    panic!("component {hash} is not loaded");
                }
                let program = child
                    .subgraphs
                    .get(subgraph)
                    .unwrap_or_else(|| panic!("component subgraph {subgraph} is not loaded"));
                let parent_memory_offset = self.pc.memory_offset;
                let child_memory_offset = self.memory_stack.len();
                self.memory_stack
                    .resize(child_memory_offset + child.memory_size, 0);
                for &input_port in &program.inputs {
                    if let (Some(&input), Some(Some(address))) =
                        (child.inputs.get(input_port), inputs.get(input_port))
                    {
                        let value = self.memory_stack[parent_memory_offset + *address];
                        self.memory_stack[child_memory_offset + input] |= value;
                    }
                }
                self.returns.push(self.pc.clone());
                self.pc = Pc {
                    instruction_index: 0,
                    subgraph: Some(subgraph),
                    memory_offset: child_memory_offset,
                    storage_offset: self.pc.storage_offset + storage_offset,
                    component: child,
                };
            }
            Instruction::Not { input, output } => {
                let offset = self.pc.memory_offset;
                self.memory_stack[offset + output] |= !self.memory_stack[offset + input];
                self.pc.instruction_index += 1;
            }
            Instruction::CopyBits {
                input,
                output,
                shift,
                mask,
            } => {
                let offset = self.pc.memory_offset;
                let value = self.memory_stack[offset + input] & mask;
                self.memory_stack[offset + output] |= if shift >= 0 {
                    value << shift
                } else {
                    self.memory_stack[offset + input] >> -shift & mask
                };
                self.pc.instruction_index += 1;
            }
            Instruction::ReadStorage { storage, output } => {
                let memory_offset = self.pc.memory_offset;
                let storage_offset = self.pc.storage_offset;
                self.memory_stack[memory_offset + output] |= self.storage[storage_offset + storage];
                self.pc.instruction_index += 1;
            }
            Instruction::SaveStorage { storage, input } => {
                let memory_offset = self.pc.memory_offset;
                let storage_offset = self.pc.storage_offset;
                self.storage[storage_offset + storage] = self.memory_stack[memory_offset + input];
                self.pc.instruction_index += 1;
            }
        }
    }

    fn return_from_component(&mut self) {
        let Some(mut caller) = self.returns.pop() else {
            return;
        };
        let Instruction::Call {
            component,
            subgraph,
            outputs,
            ..
        } = caller.instructions()[caller.instruction_index].clone()
        else {
            unreachable!("return PC must point at a call");
        };
        let component = &caller.component.components[component];
        let program = &component.subgraphs[subgraph];
        for &output_port in &program.outputs {
            if let (Some(&output), Some(Some(address))) =
                (component.outputs.get(output_port), outputs.get(output_port))
            {
                let value = self.memory_stack[self.pc.memory_offset + output];
                self.memory_stack[caller.memory_offset + address] |= value;
            }
        }
        self.memory_stack.truncate(self.pc.memory_offset);
        caller.instruction_index += 1;
        self.pc = caller;
    }

    pub fn input_addresses(&self) -> &[MemoryAddress] {
        &self.root_component.inputs
    }

    pub fn output_addresses(&self) -> &[MemoryAddress] {
        &self.root_component.outputs
    }

    pub fn root_instructions(&self) -> &[Instruction] {
        &self.root_component.instructions
    }

    pub fn root_memory(&self) -> &[u64] {
        &self.memory_stack[..self.root_component.memory_size.min(self.memory_stack.len())]
    }

    pub fn root_memory_mut(&mut self) -> &mut [u64] {
        let len = self.root_component.memory_size;
        if self.memory_stack.len() < len {
            self.memory_stack.resize(len, 0);
        }
        &mut self.memory_stack[..len]
    }
}

impl Component {
    pub fn unresolved(hash: ComponentHash) -> Self {
        Self {
            source_hash: Some(hash),
            ..Self::default()
        }
    }

    pub fn total_instruction_count(&self) -> usize {
        self.instruction_count_for(&self.instructions)
    }

    fn subgraph_instruction_count(&self, subgraph: usize) -> usize {
        self.instruction_count_for(&self.subgraphs[subgraph].instructions)
    }

    fn instruction_count_for(&self, instructions: &[Instruction]) -> usize {
        instructions
            .iter()
            .map(|instruction| match instruction {
                Instruction::Call {
                    component,
                    subgraph,
                    ..
                } => 1 + self.components[*component].subgraph_instruction_count(*subgraph),
                _ => 1,
            })
            .sum()
    }

    pub fn total_latency(&self) -> usize {
        self.latency_for(&self.instructions)
    }

    fn subgraph_latency(&self, subgraph: usize) -> usize {
        self.latency_for(&self.subgraphs[subgraph].instructions)
    }

    fn latency_for(&self, instructions: &[Instruction]) -> usize {
        let mut ready = vec![0; self.memory_size];
        let mut total = 0;
        for instruction in instructions {
            let (inputs, outputs, cost) = match instruction {
                Instruction::Call {
                    component,
                    subgraph,
                    inputs,
                    outputs,
                    ..
                } => (
                    inputs.iter().flatten().copied().collect::<Vec<_>>(),
                    outputs.iter().flatten().copied().collect::<Vec<_>>(),
                    self.components[*component].subgraph_latency(*subgraph),
                ),
                Instruction::Not { input, output } => (vec![*input], vec![*output], 1),
                Instruction::CopyBits { input, output, .. } => (vec![*input], vec![*output], 1),
                Instruction::ReadStorage { output, .. } => (Vec::new(), vec![*output], 1),
                Instruction::SaveStorage { input, .. } => (vec![*input], Vec::new(), 1),
            };
            let start = inputs
                .iter()
                .filter_map(|&input| ready.get(input))
                .copied()
                .max()
                .unwrap_or_default();
            let finish = start + cost;
            total = total.max(finish);
            for output in outputs {
                if let Some(output_ready) = ready.get_mut(output) {
                    *output_ready = (*output_ready).max(finish);
                }
            }
        }
        total
    }

    fn unresolved_hash(&self) -> Option<&ComponentHash> {
        (self.source_hash.is_some()
            && self.memory_size == 0
            && self.storage_init.is_empty()
            && self.inputs.is_empty()
            && self.outputs.is_empty()
            && self.instructions.is_empty()
            && self.subgraphs.is_empty())
        .then(|| self.source_hash.as_ref().unwrap())
    }

    fn to_unlinked(&self) -> UnlinkedComponent {
        UnlinkedComponent {
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            components: self
                .components
                .iter()
                .map(|component| {
                    component
                        .source_hash
                        .clone()
                        .expect("linked components loaded from files keep their source hash")
                })
                .collect(),
            memory_size: self.memory_size,
            storage_init: self.direct_storage_init().to_vec(),
            instructions: self.instructions.clone(),
            subgraphs: self
                .subgraphs
                .iter()
                .map(|subgraph| UnlinkedSubgraph {
                    inputs: subgraph.inputs.clone(),
                    outputs: subgraph.outputs.clone(),
                    instructions: subgraph.instructions.clone(),
                })
                .collect(),
        }
    }

    fn direct_storage_init(&self) -> &[u64] {
        let mut direct_len = self.storage_init.len();
        for instruction in &self.instructions {
            if let Instruction::Call { storage_offset, .. } = instruction {
                direct_len = direct_len.min(*storage_offset);
            }
        }
        &self.storage_init[..direct_len]
    }
}

impl Pc {
    pub fn instructions(&self) -> &[Instruction] {
        match self.subgraph {
            Some(subgraph) => &self.component.subgraphs[subgraph].instructions,
            None => &self.component.instructions,
        }
    }
}

fn link_unlinked_component<E>(
    component: &UnlinkedComponent,
    source_hash: Option<ComponentHash>,
    load: &mut impl FnMut(&ComponentHash) -> Result<Rc<Component>, E>,
) -> Result<Rc<Component>, E> {
    let mut storage_init = component.storage_init.clone();
    let mut components = Vec::with_capacity(component.components.len());
    for hash in &component.components {
        components.push(load(hash)?);
    }
    let mut instructions = component.instructions.clone();
    let mut storage_offsets = BTreeMap::<(usize, usize), StorageId>::new();
    link_instruction_storage(
        &mut instructions,
        &components,
        &mut storage_init,
        &mut storage_offsets,
    );
    let mut subgraphs = component
        .subgraphs
        .iter()
        .map(|subgraph| ComponentExecutionSubgraph {
            inputs: subgraph.inputs.clone(),
            outputs: subgraph.outputs.clone(),
            instructions: subgraph.instructions.clone(),
        })
        .collect::<Vec<_>>();
    for subgraph in &mut subgraphs {
        link_instruction_storage(
            &mut subgraph.instructions,
            &components,
            &mut storage_init,
            &mut storage_offsets,
        );
    }
    Ok(Rc::new(Component {
        inputs: component.inputs.clone(),
        outputs: component.outputs.clone(),
        components,
        instructions,
        subgraphs,
        memory_size: component.memory_size,
        storage_init,
        source_hash,
    }))
}

fn link_instruction_storage(
    instructions: &mut [Instruction],
    components: &[Rc<Component>],
    storage_init: &mut Vec<u64>,
    storage_offsets: &mut BTreeMap<(usize, usize), StorageId>,
) {
    for instruction in instructions {
        if let Instruction::Call {
            component,
            instance,
            storage_offset,
            ..
        } = instruction
        {
            let key = (*component, *instance);
            *storage_offset = *storage_offsets.entry(key).or_insert_with(|| {
                let offset = storage_init.len();
                storage_init.extend_from_slice(&components[*component].storage_init);
                offset
            });
        }
    }
}

#[derive(Clone, Debug)]
struct Operation {
    inputs: Vec<MemoryAddress>,
    outputs: Vec<MemoryAddress>,
    instructions: Vec<Instruction>,
}

fn build_subgraphs(
    operations: &[Operation],
    order: &[usize],
    public_inputs: &[MemoryAddress],
    public_outputs: &[MemoryAddress],
) -> Vec<UnlinkedSubgraph> {
    let mut writers = BTreeMap::<MemoryAddress, Vec<usize>>::new();
    for (operation_index, operation) in operations.iter().enumerate() {
        for &output in &operation.outputs {
            writers.entry(output).or_default().push(operation_index);
        }
    }

    let input_ports = public_inputs
        .iter()
        .copied()
        .enumerate()
        .map(|(port, address)| (address, port))
        .collect::<BTreeMap<_, _>>();
    let output_ports = public_outputs
        .iter()
        .copied()
        .enumerate()
        .map(|(port, address)| (address, port))
        .collect::<BTreeMap<_, _>>();

    let mut groups = BTreeMap::<Vec<usize>, (BTreeSet<usize>, BTreeSet<usize>)>::new();
    for (output_port, &output_address) in public_outputs.iter().enumerate() {
        for &writer in writers.get(&output_address).into_iter().flatten() {
            let cone = dependency_cone(operations, &writers, writer);
            groups.entry(cone).or_default().1.insert(output_port);
        }
    }
    for (operation_index, operation) in operations.iter().enumerate() {
        if operation.outputs.is_empty() {
            let cone = dependency_cone(operations, &writers, operation_index);
            groups.entry(cone).or_default();
        }
    }

    let order_index = order
        .iter()
        .copied()
        .enumerate()
        .map(|(index, operation)| (operation, index))
        .collect::<BTreeMap<_, _>>();
    let mut subgraphs = groups
        .into_iter()
        .map(|(mut cone, (mut explicit_inputs, mut explicit_outputs))| {
            cone.sort_by_key(|operation| order_index[operation]);
            for &operation_index in &cone {
                let operation = &operations[operation_index];
                for &input in &operation.inputs {
                    if let Some(&port) = input_ports.get(&input) {
                        explicit_inputs.insert(port);
                    }
                }
                for &output in &operation.outputs {
                    if let Some(&port) = output_ports.get(&output) {
                        explicit_outputs.insert(port);
                    }
                }
            }
            UnlinkedSubgraph {
                inputs: explicit_inputs.into_iter().collect(),
                outputs: explicit_outputs.into_iter().collect(),
                instructions: cone
                    .into_iter()
                    .flat_map(|operation| operations[operation].instructions.clone())
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    subgraphs.sort_by(|left, right| {
        left.outputs
            .cmp(&right.outputs)
            .then_with(|| left.inputs.cmp(&right.inputs))
    });
    if subgraphs.is_empty() && (!public_inputs.is_empty() || !public_outputs.is_empty()) {
        subgraphs.push(UnlinkedSubgraph {
            inputs: (0..public_inputs.len()).collect(),
            outputs: (0..public_outputs.len()).collect(),
            instructions: Vec::new(),
        });
    }
    subgraphs
}

fn dependency_cone(
    operations: &[Operation],
    writers: &BTreeMap<MemoryAddress, Vec<usize>>,
    root: usize,
) -> Vec<usize> {
    let mut pending = vec![root];
    let mut cone = BTreeSet::new();
    while let Some(operation_index) = pending.pop() {
        if !cone.insert(operation_index) {
            continue;
        }
        for &input in &operations[operation_index].inputs {
            for &writer in writers.get(&input).into_iter().flatten() {
                pending.push(writer);
            }
        }
    }
    cone.into_iter().collect()
}

fn dense_input_indices(grid: &LogicGrid) -> BTreeMap<InputId, usize> {
    dense_port_indices(
        grid.components()
            .filter_map(|component| match component.kind {
                ComponentKind::Input { id, .. } => Some(id),
                _ => None,
            }),
    )
}

fn dense_output_indices(grid: &LogicGrid) -> BTreeMap<OutputId, usize> {
    dense_port_indices(
        grid.components()
            .filter_map(|component| match component.kind {
                ComponentKind::Output { id, .. } => Some(id),
                _ => None,
            }),
    )
}

fn dense_port_indices<T: Ord>(ids: impl IntoIterator<Item = T>) -> BTreeMap<T, usize> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect()
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
mod tests;
