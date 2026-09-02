use super::*;

impl LogicGridEditor {
    pub(super) fn show_metrics(&self, context: &egui::Context) {
        let bounds = self.grid.bounds();

        egui::Window::new("Metrics")
            .default_pos([700.0, 16.0])
            .default_width(240.0)
            .show(context, |ui| {
                if let Some(error) = &self.simulation.error {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Cannot compile: {error}"),
                    );
                    ui.separator();
                }

                egui::Grid::new("logic-metrics-summary")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Total instructions");
                        match &self.simulation.vm {
                            Some(vm) => {
                                ui.monospace(
                                    vm.root_component.total_instruction_count().to_string(),
                                );
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();

                        ui.label("Total latency");
                        match &self.simulation.vm {
                            Some(vm) => {
                                ui.monospace(vm.root_component.total_latency().to_string());
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();

                        ui.label("Total area");
                        match bounds {
                            Some(bounds) => {
                                ui.monospace(format!(
                                    "{} ({} x {})",
                                    bounds.area(),
                                    bounds.width(),
                                    bounds.height()
                                ));
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();
                    });
            });
    }

    pub(super) fn show_simulation(&mut self, context: &egui::Context) {
        egui::Window::new("Simulation")
            .default_pos([16.0, 390.0])
            .default_width(300.0)
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Run tick").clicked() {
                        self.run_simulation_tick();
                    }
                    if ui.button("Step instruction").clicked() {
                        self.run_simulation_instruction();
                    }
                });

                ui.separator();
                if let Some(error) = &self.simulation.error {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Cannot run: {error}"));
                    return;
                }

                let Some(vm) = &mut self.simulation.vm else {
                    ui.weak("Run a step to compile and execute the circuit.");
                    return;
                };
                let Some(snapshot) = &self.simulation.snapshot else {
                    return;
                };
                let instruction_selection = self.simulation.instruction_selection.clone();
                let instruction_view = simulation_instruction_view(
                    vm,
                    &instruction_selection,
                    self.simulation.tick_in_progress,
                );

                egui::Grid::new("logic-simulation-summary")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Steps");
                        ui.monospace(self.simulation.steps.to_string());
                        ui.end_row();
                        ui.label("Instructions");
                        ui.monospace(instruction_view.instructions.len().to_string());
                        ui.end_row();
                        ui.label("Viewing");
                        ui.monospace(&instruction_view.name);
                        ui.end_row();
                        ui.label("Next here");
                        if let Some(next_instruction) = instruction_view.next_instruction {
                            ui.monospace(format!(
                                "{} / {}",
                                next_instruction + 1,
                                instruction_view.instructions.len()
                            ));
                        } else {
                            if instruction_view.instructions.is_empty() {
                                ui.weak("none");
                            } else {
                                ui.weak("not active");
                            }
                        }
                        ui.end_row();
                    });

                ui.separator();
                ui.strong("Call stack");
                let root_active = matches!(
                    self.simulation.instruction_selection,
                    SimulationInstructionSelection::ReturnFrame(0)
                ) || vm.returns.is_empty()
                    && matches!(
                        self.simulation.instruction_selection,
                        SimulationInstructionSelection::Active
                    );
                if ui.selectable_label(root_active, "Root").clicked() {
                    self.simulation.instruction_selection = if vm.returns.is_empty() {
                        SimulationInstructionSelection::Active
                    } else {
                        SimulationInstructionSelection::ReturnFrame(0)
                    };
                }
                for (index, pc) in vm.returns.iter().enumerate().skip(1) {
                    if ui
                        .selectable_label(
                            matches!(
                                self.simulation.instruction_selection,
                                SimulationInstructionSelection::ReturnFrame(selected)
                                    if selected == index
                            ),
                            format!(
                                "Caller {index}: {}",
                                simulation_component_name(&pc.component)
                            ),
                        )
                        .clicked()
                    {
                        self.simulation.instruction_selection =
                            SimulationInstructionSelection::ReturnFrame(index);
                    }
                }
                if !vm.returns.is_empty()
                    && ui
                        .selectable_label(
                            matches!(
                                self.simulation.instruction_selection,
                                SimulationInstructionSelection::Active
                            ),
                            format!("Current: {}", simulation_component_name(&vm.pc.component)),
                        )
                        .clicked()
                {
                    self.simulation.instruction_selection = SimulationInstructionSelection::Active;
                }

                ui.separator();
                ui.strong("Instructions");
                if instruction_view.instructions.is_empty() {
                    ui.weak("No instructions");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("logic-simulation-instructions")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, instruction) in
                                instruction_view.instructions.iter().enumerate()
                            {
                                let next = instruction_view.next_instruction == Some(index);
                                let response = ui.horizontal(|ui| {
                                    let response = ui.selectable_label(
                                        next,
                                        egui::RichText::new(format!(
                                            "{index:03}  {}",
                                            format_instruction(instruction)
                                        ))
                                        .monospace(),
                                    );
                                    if let Instruction::Call { component, .. } = instruction {
                                        if let Some(component) =
                                            instruction_view.component.components.get(*component)
                                        {
                                            if ui.small_button("target").clicked() {
                                                self.simulation.instruction_selection =
                                                    SimulationInstructionSelection::Component(
                                                        Rc::clone(component),
                                                    );
                                            }
                                        } else {
                                            ui.add_enabled(false, egui::Button::new("target"));
                                        }
                                    }
                                    response
                                });
                                if next {
                                    response.inner.scroll_to_me(Some(egui::Align::Center));
                                }
                            }
                        });
                }

                ui.separator();
                ui.strong("Inputs");
                if vm.input_addresses().is_empty() {
                    ui.weak("No input components");
                } else {
                    let input_addresses = vm.input_addresses().to_vec();
                    let mut input_ports = snapshot
                        .components
                        .iter()
                        .filter_map(|component| match component.kind {
                            ComponentKind::Input { scale, id, .. } => Some((id, scale)),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    input_ports.sort_by_key(|(id, _)| *id);
                    for (input, address) in input_addresses.into_iter().enumerate() {
                        let Some(value) = self.simulation.input_values.get_mut(input) else {
                            ui.horizontal(|ui| {
                                ui.label(format!("Input {input}"));
                                ui.weak("deleted");
                            });
                            continue;
                        };
                        let scale = input_ports.get(input).map(|(_, scale)| *scale);
                        ui.horizontal(|ui| {
                            ui.label(format!("Input {input}"));
                            if let Some(scale) = scale {
                                if address >= vm.root_component.memory_size {
                                    ui.weak("deleted");
                                    return;
                                }
                                for bit in storage_bit_indices(scale) {
                                    let state = (*value >> bit) & 1;
                                    if ui.small_button(format!("{bit}:{state}")).clicked() {
                                        *value ^= 1_u64 << bit;
                                        vm.root_memory_mut()[address] |= *value;
                                    }
                                }
                            } else {
                                ui.weak("deleted");
                            }
                        });
                    }
                }

                ui.separator();
                ui.strong("Outputs");
                if vm.output_addresses().is_empty() {
                    ui.weak("No output components");
                } else {
                    let output_count = snapshot
                        .components
                        .iter()
                        .filter(|component| matches!(component.kind, ComponentKind::Output { .. }))
                        .count();
                    for (output, &address) in vm.output_addresses().iter().enumerate() {
                        let exists = output < output_count;
                        if exists {
                            if let Some(&value) = vm.root_memory().get(address) {
                                simulation_value_row(ui, format!("Output {output}"), value);
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Output {output}"));
                                    ui.weak("deleted");
                                });
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(format!("Output {output}"));
                                ui.weak("deleted");
                            });
                        }
                    }
                }

                ui.separator();
                ui.strong("Wire groups");
                if vm.root_memory().is_empty() {
                    ui.weak("No connected wire groups");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("logic-simulation-wires")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (address, value) in vm.root_memory().iter().copied().enumerate() {
                                let segment_count = snapshot
                                    .graph
                                    .nodes
                                    .iter()
                                    .filter_map(|node| match node {
                                        GraphNode::WireNet { wires } => Some(wires.len()),
                                        _ => None,
                                    })
                                    .nth(address)
                                    .unwrap_or_default();
                                simulation_value_row(
                                    ui,
                                    format!("Memory {address} ({segment_count} segments)"),
                                    value,
                                );
                            }
                        });
                }

                ui.separator();
                ui.strong("Storage");
                if vm.storage.is_empty() {
                    ui.weak("No storage components");
                } else {
                    for (storage, value) in vm.storage.iter().copied().enumerate() {
                        let component = snapshot
                            .components
                            .iter()
                            .filter(|component| {
                                matches!(component.kind, ComponentKind::Storage { .. })
                            })
                            .nth(storage)
                            .map(|component| component.id);
                        let label = component
                            .map(|component| format!("Storage #{}", component.0))
                            .unwrap_or_else(|| format!("Storage {storage}"));
                        simulation_value_row(ui, label, value);
                    }
                }
            });
    }

    pub(super) fn run_simulation_tick(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        while self.simulation.tick_in_progress {
            self.execute_next_simulation_instruction();
        }
    }

    fn link_called_components(&self, vm: &mut Vm) -> Result<(), String> {
        let mut cache = BTreeMap::<Uuid, Rc<ExecutionComponent>>::new();
        vm.load_components(|called| self.link_compiled(called, &mut cache))
    }

    fn link_compiled(
        &self,
        compiled: Uuid,
        cache: &mut BTreeMap<Uuid, Rc<ExecutionComponent>>,
    ) -> Result<Rc<ExecutionComponent>, String> {
        if let Some(component) = cache.get(&compiled) {
            return Ok(Rc::clone(component));
        }
        let program = self
            .compiled
            .get(&compiled)
            .and_then(BlockHandle::read)
            .ok_or_else(|| format!("component {compiled} has not loaded yet"))?;
        let linked = program
            .program()
            .link_with_source(compiled, |called| self.link_compiled(called, cache))?;
        drop(program);
        cache.insert(compiled, Rc::clone(&linked));
        Ok(linked)
    }

    pub(super) fn run_simulation_instruction(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        self.execute_next_simulation_instruction();
    }

    pub(super) fn compile_simulation(&mut self, snapshot: SimulationSnapshot) {
        let previous_input_values = self.simulation.input_values.clone();
        match Vm::from_graph(&self.grid, &snapshot.graph).map_err(|error| format!("{error:?}")) {
            Ok(mut vm) => {
                if let Err(error) = self.link_called_components(&mut vm) {
                    self.simulation = Simulation {
                        snapshot: Some(snapshot),
                        vm: None,
                        error: Some(error),
                        input_values: Vec::new(),
                        steps: 0,
                        instruction_selection: SimulationInstructionSelection::Active,
                        tick_in_progress: false,
                    };
                    return;
                }
                let mut input_values = vec![0; vm.input_addresses().len()];
                for (input, value) in input_values.iter_mut().zip(previous_input_values) {
                    *input = value;
                }
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    input_values,
                    vm: Some(vm),
                    error: None,
                    steps: 0,
                    instruction_selection: SimulationInstructionSelection::Active,
                    tick_in_progress: false,
                };
            }
            Err(error) => {
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    vm: None,
                    error: Some(error),
                    input_values: Vec::new(),
                    steps: 0,
                    instruction_selection: SimulationInstructionSelection::Active,
                    tick_in_progress: false,
                };
            }
        }
    }

    pub(super) fn prepare_simulation(&mut self) -> bool {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
        }
        self.simulation.vm.is_some()
    }

    pub(super) fn update_simulation_preview(&mut self) {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
            self.run_simulation_tick();
        }
    }

    pub(super) fn begin_simulation_tick(&mut self) -> bool {
        if self.simulation.tick_in_progress {
            return true;
        }
        let Some(vm) = &mut self.simulation.vm else {
            return false;
        };
        vm.begin_tick();
        apply_input_values(vm, &self.simulation.input_values);
        self.simulation.instruction_selection = SimulationInstructionSelection::Active;
        if vm.root_instructions().is_empty() {
            self.simulation.steps += 1;
            return false;
        }
        self.simulation.tick_in_progress = true;
        true
    }

    pub(super) fn execute_next_simulation_instruction(&mut self) {
        let Some(vm) = &mut self.simulation.vm else {
            return;
        };
        vm.execute_instruction();
        self.simulation.instruction_selection = SimulationInstructionSelection::Active;
        if vm.is_tick_complete() {
            self.simulation.steps += 1;
            self.simulation.tick_in_progress = false;
        }
    }

    pub(super) fn simulation_snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            components: self.grid.components().cloned().collect(),
            wires: self.grid.wires().to_vec(),
            graph: self.grid.generate_graph(),
        }
    }
}
pub(super) fn storage_bit_indices(scale: Scale) -> Vec<u32> {
    (0..scale.get() as u32).rev().collect()
}

pub(super) struct SimulationInstructionView<'a> {
    pub(super) name: String,
    pub(super) component: &'a Rc<ExecutionComponent>,
    pub(super) instructions: &'a [Instruction],
    pub(super) next_instruction: Option<usize>,
}

pub(super) fn simulation_instruction_view<'a>(
    vm: &'a Vm,
    selection: &'a SimulationInstructionSelection,
    tick_in_progress: bool,
) -> SimulationInstructionView<'a> {
    match selection {
        SimulationInstructionSelection::ReturnFrame(index) => vm
            .returns
            .get(*index)
            .map(|pc| simulation_pc_instruction_view("Caller", pc, true))
            .unwrap_or_else(|| simulation_pc_instruction_view("Current", &vm.pc, tick_in_progress)),
        SimulationInstructionSelection::Component(component) => SimulationInstructionView {
            name: format!("Target: {}", simulation_component_name(component)),
            component,
            instructions: &component.instructions,
            next_instruction: None,
        },
        SimulationInstructionSelection::Active => {
            simulation_pc_instruction_view("Current", &vm.pc, tick_in_progress)
        }
    }
}

pub(super) fn simulation_pc_instruction_view<'a>(
    name: &str,
    pc: &'a Pc,
    active: bool,
) -> SimulationInstructionView<'a> {
    SimulationInstructionView {
        name: format!("{name}: {}", simulation_component_name(&pc.component)),
        component: &pc.component,
        instructions: pc.instructions(),
        next_instruction: (active && pc.instruction_index < pc.instructions().len())
            .then_some(pc.instruction_index),
    }
}

pub(super) fn simulation_component_name(component: &ExecutionComponent) -> String {
    component
        .source
        .map_or_else(|| "component".to_owned(), |source| source.to_string())
}

pub(super) fn simulation_value_row(ui: &mut egui::Ui, label: String, value: u64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.monospace(format!("0x{value:016x}"));
        ui.weak(format!("({value})"));
    });
}

pub(super) fn apply_input_values(vm: &mut Vm, values: &[u64]) {
    for (&address, value) in vm.input_addresses().to_vec().iter().zip(values) {
        if address < vm.root_component.memory_size {
            vm.root_memory_mut()[address] |= *value;
        }
    }
}

pub(super) fn format_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Call {
            component,
            instance,
            subgraph,
            inputs,
            outputs,
            ..
        } => format!("CALL c{component} i{instance} g{subgraph} {inputs:?} -> {outputs:?}"),
        Instruction::Not { input, output } => format!("NOT m{input} -> m{output}"),
        Instruction::CopyBits {
            input,
            output,
            shift,
            mask,
        } => format!("BITS m{input} shift {shift} mask {mask:#x} -> m{output}"),
        Instruction::ReadStorage { storage, output } => {
            format!("READ s{storage} -> m{output}")
        }
        Instruction::SaveStorage { storage, input } => {
            format!("SAVE m{input} -> s{storage}")
        }
    }
}
