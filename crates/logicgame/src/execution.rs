use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use serde::{Deserialize, Serialize};

use crate::grid::{
    value_mask, CircuitGraph, ComponentHash, ComponentId, ComponentKind, ConnectionDirection,
    ConnectionSlotId, GraphNode, GraphNodeId, LogicGrid,
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
    pub memory_size: usize,
    pub storage_init: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub inputs: Vec<MemoryAddress>,
    pub outputs: Vec<MemoryAddress>,
    pub components: Vec<Rc<Component>>,
    pub instructions: Vec<Instruction>,
    pub memory_size: usize,
    pub storage_init: Vec<u64>,
    pub source_hash: Option<ComponentHash>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pc {
    pub instruction_index: usize,
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
        let mut inputs = Vec::<MemoryAddress>::new();
        let mut outputs = Vec::<MemoryAddress>::new();
        for component in grid.components() {
            match &component.kind {
                ComponentKind::Storage { value, .. } => {
                    storage_ids.insert(component.id, storage_init.len());
                    storage_init.push(*value);
                }
                ComponentKind::Input { id, .. } => inputs.resize(id.0 + 1, 0),
                ComponentKind::Output { id, .. } => outputs.resize_with(id.0 + 1, Default::default),
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
                    if let Some(&address) = addresses.first() {
                        inputs[id.0] = address;
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
                    if let Some(&address) = addresses.first() {
                        outputs[id.0] = address;
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
                    let component = *component_indices
                        .entry(component_hash.clone())
                        .or_insert_with(|| {
                            let index = components.len();
                            components.push(component_hash.clone());
                            index
                        });
                    operations.push(Operation {
                        inputs: inputs.iter().flatten().copied().collect(),
                        outputs: outputs.iter().flatten().copied().collect(),
                        instructions: vec![Instruction::Call {
                            component,
                            storage_offset: 0,
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
            inputs,
            outputs,
            components,
            instructions,
            memory_size: memory_addresses.len(),
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
        if self.pc.instruction_index >= self.pc.component.instructions.len() {
            self.return_from_component();
            return;
        }

        match self.pc.component.instructions[self.pc.instruction_index].clone() {
            Instruction::Call {
                component,
                storage_offset,
                inputs,
                ..
            } => {
                let child = Rc::clone(&self.pc.component.components[component]);
                if let Some(hash) = child.unresolved_hash() {
                    panic!("component {hash} is not loaded");
                }
                let parent_memory_offset = self.pc.memory_offset;
                let child_memory_offset = self.memory_stack.len();
                self.memory_stack
                    .resize(child_memory_offset + child.memory_size, 0);
                for (&input, binding) in child.inputs.iter().zip(&inputs) {
                    if let Some(address) = binding {
                        let value = self.memory_stack[parent_memory_offset + *address];
                        self.memory_stack[child_memory_offset + input] |= value;
                    }
                }
                self.returns.push(self.pc.clone());
                self.pc = Pc {
                    instruction_index: 0,
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
            component, outputs, ..
        } = &caller.component.instructions[caller.instruction_index]
        else {
            unreachable!("return PC must point at a call");
        };
        let component = &caller.component.components[*component];
        for (&output, binding) in component.outputs.iter().zip(outputs) {
            if let Some(address) = binding {
                let value = self.memory_stack[self.pc.memory_offset + output];
                self.memory_stack[caller.memory_offset + *address] |= value;
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
        self.instructions
            .iter()
            .map(|instruction| match instruction {
                Instruction::Call { component, .. } => {
                    1 + self.components[*component].total_instruction_count()
                }
                _ => 1,
            })
            .sum()
    }

    pub fn total_latency(&self) -> usize {
        let mut ready = vec![0; self.memory_size];
        let mut total = 0;
        for instruction in &self.instructions {
            let (inputs, outputs, cost) = match instruction {
                Instruction::Call {
                    component,
                    inputs,
                    outputs,
                    ..
                } => (
                    inputs.iter().flatten().copied().collect::<Vec<_>>(),
                    outputs.iter().flatten().copied().collect::<Vec<_>>(),
                    self.components[*component].total_latency(),
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
            && self.instructions.is_empty())
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
    for instruction in &mut instructions {
        if let Instruction::Call {
            component,
            storage_offset,
            ..
        } = instruction
        {
            let child = &components[*component];
            *storage_offset = storage_init.len();
            storage_init.extend_from_slice(&child.storage_init);
        }
    }
    Ok(Rc::new(Component {
        inputs: component.inputs.clone(),
        outputs: component.outputs.clone(),
        components,
        instructions,
        memory_size: component.memory_size,
        storage_init,
        source_hash,
    }))
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
        Rc::new(Component {
            memory_size,
            storage_init,
            inputs,
            outputs,
            components,
            instructions,
            source_hash: None,
        })
    }

    fn vm_with_root(root: Rc<Component>) -> Vm {
        let mut vm = Vm::from_unlinked_component(root);
        vm.begin_tick();
        vm
    }

    #[test]
    fn total_instruction_count_includes_called_components_per_call() {
        let child = component(
            1,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            (0..10)
                .map(|_| Instruction::Not {
                    input: 0,
                    output: 0,
                })
                .collect(),
        );
        let root = component_with_children(
            1,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![child],
            vec![
                Instruction::Not {
                    input: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 0,
                },
                Instruction::Call {
                    component: 0,
                    storage_offset: 0,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                },
                Instruction::Call {
                    component: 0,
                    storage_offset: 0,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                },
                Instruction::Not {
                    input: 0,
                    output: 0,
                },
            ],
        );

        assert_eq!(root.total_instruction_count(), 25);
    }

    #[test]
    fn total_latency_counts_dependent_gate_chain() {
        let root = component(
            3,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::Not {
                    input: 1,
                    output: 2,
                },
            ],
        );

        assert_eq!(root.total_latency(), 2);
    }

    #[test]
    fn total_latency_uses_parallel_branch_depth() {
        let root = component(
            4,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![
                Instruction::Not {
                    input: 0,
                    output: 2,
                },
                Instruction::Not {
                    input: 1,
                    output: 3,
                },
            ],
        );

        assert_eq!(root.total_latency(), 1);
    }

    #[test]
    fn total_latency_includes_subcomponent_latency() {
        let child = component(
            3,
            Vec::new(),
            vec![0],
            vec![2],
            vec![
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::Not {
                    input: 1,
                    output: 2,
                },
            ],
        );
        let root = component_with_children(
            2,
            Vec::new(),
            vec![0],
            vec![1],
            vec![child],
            vec![
                Instruction::Call {
                    component: 0,
                    storage_offset: 0,
                    inputs: vec![Some(0)],
                    outputs: vec![Some(0)],
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
            ],
        );

        assert_eq!(root.total_latency(), 3);
    }

    #[test]
    fn total_latency_counts_save_storage_as_terminal_work() {
        let root = component(
            1,
            vec![0],
            Vec::new(),
            Vec::new(),
            vec![Instruction::SaveStorage {
                storage: 0,
                input: 0,
            }],
        );

        assert_eq!(root.total_latency(), 1);
    }

    #[test]
    fn not_reads_and_writes_memory() {
        let mut vm = vm_with_root(component(
            2,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![Instruction::Not {
                input: 0,
                output: 1,
            }],
        ));
        vm.memory_stack[0] = 0x00ff;

        vm.execute();

        assert_eq!(vm.root_memory(), &[0x00ff, !0x00ff]);
    }

    #[test]
    fn storage_can_be_saved_and_read() {
        let mut vm = vm_with_root(component(
            2,
            vec![0],
            Vec::new(),
            Vec::new(),
            vec![
                Instruction::SaveStorage {
                    storage: 0,
                    input: 0,
                },
                Instruction::ReadStorage {
                    storage: 0,
                    output: 1,
                },
            ],
        ));
        vm.memory_stack[0] = 42;

        vm.execute();

        assert_eq!(vm.storage, vec![42]);
        assert_eq!(vm.root_memory(), &[42, 42]);
    }

    #[test]
    fn instructions_can_execute_one_at_a_time() {
        let mut vm = vm_with_root(component(
            2,
            vec![7],
            Vec::new(),
            Vec::new(),
            vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
            ],
        ));

        vm.execute_instruction();
        assert_eq!(vm.root_memory(), &[7, 0]);

        vm.execute_instruction();
        assert_eq!(vm.root_memory(), &[7, !7]);
    }

    #[test]
    #[should_panic(expected = "is not loaded")]
    fn unloaded_call_panics() {
        let hash = ComponentHash::new("0".repeat(64)).unwrap();
        let mut vm = vm_with_root(component_with_children(
            0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![Rc::new(Component::unresolved(hash))],
            vec![Instruction::Call {
                component: 0,
                storage_offset: 0,
                inputs: vec![],
                outputs: vec![],
            }],
        ));

        vm.execute();
    }

    #[test]
    fn call_clears_memory_writes_inputs_and_outputs_through_bindings() {
        let child = component(2, Vec::new(), vec![0], vec![0], Vec::new());
        let root = component_with_children(
            2,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![child],
            vec![Instruction::Call {
                component: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![Some(1)],
            }],
        );
        let mut vm = vm_with_root(root);
        vm.memory_stack[0] = 5;

        vm.execute();
        assert_eq!(vm.root_memory(), &[5, 5]);

        vm.begin_tick();
        vm.execute();
        assert_eq!(vm.root_memory(), &[0, 0]);
    }

    #[test]
    fn repeated_subcomponents_share_code_and_have_independent_storage() {
        let hash = ComponentHash::new("3".repeat(64)).unwrap();
        let leaf_unlinked = UnlinkedComponent {
            memory_size: 1,
            storage_init: vec![0],
            inputs: vec![0],
            outputs: Vec::new(),
            components: Vec::new(),
            instructions: vec![Instruction::SaveStorage {
                storage: 0,
                input: 0,
            }],
        };
        let root_unlinked = UnlinkedComponent {
            memory_size: 2,
            storage_init: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            components: vec![hash.clone()],
            instructions: vec![
                Instruction::Call {
                    component: 0,
                    storage_offset: 0,
                    inputs: vec![Some(0)],
                    outputs: vec![],
                },
                Instruction::Call {
                    component: 0,
                    storage_offset: 0,
                    inputs: vec![Some(1)],
                    outputs: vec![],
                },
            ],
        };
        let mut cache = BTreeMap::<ComponentHash, Rc<Component>>::new();
        let root = root_unlinked
            .link(|requested| {
                Ok::<_, ()>(
                    cache
                        .entry(requested.clone())
                        .or_insert_with(|| {
                            leaf_unlinked
                                .link_with_hash(
                                    requested.clone(),
                                    |_| -> Result<Rc<Component>, ()> {
                                        panic!("leaf has no child components")
                                    },
                                )
                                .unwrap()
                        })
                        .clone(),
                )
            })
            .unwrap();
        assert_eq!(root.components.len(), 1);

        let mut vm = vm_with_root(root);
        vm.memory_stack.copy_from_slice(&[1, 2]);
        vm.execute();

        assert_eq!(vm.storage, vec![1, 2]);
    }

    #[test]
    fn nested_subcomponent_storage_is_owned_by_the_root_vm() {
        let leaf_hash = ComponentHash::new("5".repeat(64)).unwrap();
        let leaf = UnlinkedComponent {
            memory_size: 1,
            storage_init: vec![8],
            inputs: vec![0],
            outputs: Vec::new(),
            components: Vec::new(),
            instructions: vec![Instruction::SaveStorage {
                storage: 0,
                input: 0,
            }],
        };
        let middle = UnlinkedComponent {
            memory_size: 1,
            storage_init: Vec::new(),
            inputs: vec![0],
            outputs: Vec::new(),
            components: vec![leaf_hash.clone()],
            instructions: vec![Instruction::Call {
                component: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![],
            }],
        };
        let middle = middle
            .link(|hash| -> Result<Rc<Component>, ()> {
                assert_eq!(hash, &leaf_hash);
                leaf.link_with_hash(hash.clone(), |_| panic!("leaf has no child components"))
            })
            .unwrap();
        let root = component_with_children(
            1,
            middle.storage_init.clone(),
            Vec::new(),
            Vec::new(),
            vec![middle],
            vec![Instruction::Call {
                component: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![],
            }],
        );
        let mut vm = vm_with_root(root);
        vm.memory_stack[0] = 13;

        vm.execute();

        assert_eq!(vm.storage, vec![13]);
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

        let component = UnlinkedComponent::from_graph(&grid, &graph).unwrap();

        assert_eq!(component.memory_size, 3);
        assert_eq!(component.storage_init, vec![1, 0]);
        assert_eq!(
            component.instructions,
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
    fn inputs_and_outputs_compile_to_memory_bindings() {
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
        assert_eq!(vm.input_addresses(), &[0, 0]);
        assert_eq!(vm.output_addresses(), &[0, 0]);
        assert!(vm.root_instructions().is_empty());

        vm.begin_tick();
        let input_address = vm.input_addresses()[1];
        vm.root_memory_mut()[input_address] |= 0xff;
        vm.execute();
        assert_eq!(vm.root_memory(), &[0xff]);
        assert_eq!(vm.root_memory()[vm.output_addresses()[1]], 0xff);

        vm.root_memory_mut()[0] = u64::MAX;
        vm.begin_tick();
        assert_eq!(vm.root_memory(), &[0]);
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
        vm.begin_tick();
        vm.root_memory_mut()[0] = 0xabcd;

        vm.execute();

        assert_eq!(vm.root_memory(), &[0xabcd, 0xd, 0xc, 0xb, 0xa]);
    }

    #[test]
    fn subcomponents_compile_sparse_bindings() {
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
                    second: GraphNodeId(2),
                },
                GraphEdge {
                    first: GraphNodeId(1),
                    second: GraphNodeId(3),
                },
            ],
        };

        let component = UnlinkedComponent::from_graph(&grid, &graph).unwrap();

        assert_eq!(component.components, vec![component_hash]);
        assert_eq!(
            component.instructions,
            vec![Instruction::Call {
                component: 0,
                storage_offset: 0,
                inputs: vec![None, Some(0)],
                outputs: vec![None, None, Some(1)],
            }]
        );
    }
}
