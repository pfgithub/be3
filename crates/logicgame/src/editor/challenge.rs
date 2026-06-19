use super::*;

impl LogicEditor {
    pub fn open_challenge_solution(&mut self, id: ChallengeId, grid: LogicGrid) {
        self.replace_grid(grid);
        self.tool = Tool {
            kind: ToolKind::Select,
            scale: Scale::ONE,
            merger_out_scale: Scale::ONE,
        };
        self.challenge = Some(ChallengeState {
            id,
            data: generate_challenge(id),
            test: ChallengeTest::default(),
            passed_event: false,
        });
        self.set_context_hotbar_folder(("Challenge", default_component_slots()));
    }

    pub fn active_challenge_id(&self) -> Option<ChallengeId> {
        self.challenge.as_ref().map(|challenge| challenge.id)
    }

    pub(super) fn next_missing_challenge_input(&self) -> Option<(usize, Scale, String)> {
        let challenge = self.challenge.as_ref()?;
        challenge
            .data
            .inputs
            .iter()
            .enumerate()
            .find(|(index, _)| {
                let id = InputId::from_u128(*index as u128);
                self.grid.components().all(|component| {
                    !matches!(component.kind, ComponentKind::Input { id: placed, .. } if placed == id)
                })
            })
            .map(|(index, port)| (index, port.scale, port.label.to_owned()))
    }

    pub(super) fn next_missing_challenge_output(&self) -> Option<(usize, Scale, String)> {
        let challenge = self.challenge.as_ref()?;
        challenge
            .data
            .outputs
            .iter()
            .enumerate()
            .find(|(index, _)| {
                let id = OutputId::from_u128(*index as u128);
                self.grid.components().all(|component| {
                    !matches!(component.kind, ComponentKind::Output { id: placed, .. } if placed == id)
                })
            })
            .map(|(index, port)| (index, port.scale, port.label.to_owned()))
    }

    pub(super) fn active_input_scale(&self) -> Scale {
        self.next_missing_challenge_input()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    pub(super) fn active_output_scale(&self) -> Scale {
        self.next_missing_challenge_output()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    pub(super) fn active_tool_snap(&self) -> Scale {
        match self.tool.kind {
            ToolKind::Input => self.active_input_scale(),
            ToolKind::Output => self.active_output_scale(),
            _ => self.tool.snap(),
        }
    }

    pub(super) fn add_input_at(&mut self, position: Point, rotation: Rotation) {
        if let Some((port, scale, label)) = self.next_missing_challenge_input() {
            self.grid.add_component_with_explicit_io(
                position,
                rotation,
                ComponentKind::Input {
                    scale,
                    id: InputId::from_u128(port as u128),
                    label,
                },
            );
        } else if self.challenge.is_none() {
            self.grid.add_component(
                position,
                rotation,
                ComponentKind::Input {
                    scale: self.tool.scale,
                    id: InputId::from_u128(u128::MAX),
                    label: self.io_label.clone(),
                },
            );
        }
    }

    pub(super) fn add_output_at(&mut self, position: Point, rotation: Rotation) {
        if let Some((port, scale, label)) = self.next_missing_challenge_output() {
            self.grid.add_component_with_explicit_io(
                position,
                rotation,
                ComponentKind::Output {
                    scale,
                    id: OutputId::from_u128(port as u128),
                    label,
                },
            );
        } else if self.challenge.is_none() {
            self.grid.add_component(
                position,
                rotation,
                ComponentKind::Output {
                    scale: self.tool.scale,
                    id: OutputId::from_u128(u128::MAX),
                    label: self.io_label.clone(),
                },
            );
        }
    }

    /// Returns and clears the one-shot flag set when the challenge test passes.
    pub fn take_challenge_passed(&mut self) -> bool {
        match self.challenge.as_mut() {
            Some(challenge) => std::mem::take(&mut challenge.passed_event),
            None => false,
        }
    }

    /// Recompiles the shared simulation VM for challenge testing if the grid
    /// changed since it was built, resetting any results. Cheap when the grid
    /// is unchanged.
    pub(super) fn ensure_challenge_test(&mut self) {
        if self.challenge.is_none() {
            return;
        }
        let snapshot = self.simulation_snapshot();
        let up_to_date = self
            .challenge
            .as_ref()
            .is_some_and(|challenge| challenge.test.snapshot.as_ref() == Some(&snapshot));
        if up_to_date {
            return;
        }
        let (input_count, output_count) = match &self.challenge {
            Some(challenge) => (challenge.data.inputs.len(), challenge.data.outputs.len()),
            None => return,
        };
        let test = self.compile_challenge_test(snapshot, input_count, output_count);
        if let Some(challenge) = self.challenge.as_mut() {
            challenge.test = test;
        }
    }

    pub(super) fn compile_challenge_test(
        &mut self,
        snapshot: SimulationSnapshot,
        input_count: usize,
        output_count: usize,
    ) -> ChallengeTest {
        let input_slots = challenge_port_slots(
            self.grid
                .components()
                .filter_map(|component| match component.kind {
                    ComponentKind::Input { id, .. } => Some(id),
                    _ => None,
                }),
            input_count,
            InputId::from_u128,
        );
        let output_slots = challenge_port_slots(
            self.grid
                .components()
                .filter_map(|component| match component.kind {
                    ComponentKind::Output { id, .. } => Some(id),
                    _ => None,
                }),
            output_count,
            OutputId::from_u128,
        );

        self.compile_simulation(snapshot.clone());
        if let Some(error) = self.simulation.error.clone() {
            return ChallengeTest {
                snapshot: Some(snapshot),
                error: Some(error),
                input_slots,
                output_slots,
                actual: vec![Vec::new(); output_count],
                ..ChallengeTest::default()
            };
        }

        ChallengeTest {
            snapshot: Some(snapshot),
            error: None,
            input_slots,
            output_slots,
            next_tick: 0,
            actual: vec![Vec::new(); output_count],
            mismatched: false,
        }
    }

    pub(super) fn challenge_test_reset(&mut self) {
        if let Some(challenge) = self.challenge.as_mut() {
            // Drop the stale snapshot so the next `ensure` recompiles from scratch.
            challenge.test.snapshot = None;
        }
        self.ensure_challenge_test();
    }

    pub(super) fn challenge_test_step(&mut self) {
        self.ensure_challenge_test();
        self.advance_challenge_test_tick();
    }

    /// Re-runs the test from the start through `tick` (inclusive) so the wires
    /// reflect the inputs of that row.
    pub(super) fn challenge_test_seek(&mut self, tick: usize) {
        self.challenge_test_reset();
        for _ in 0..=tick {
            self.advance_challenge_test_tick();
        }
    }

    pub(super) fn challenge_test_run_all(&mut self) {
        self.ensure_challenge_test();
        loop {
            let more = self.challenge.as_ref().is_some_and(|challenge| {
                let test = &challenge.test;
                test.error.is_none()
                    && self.simulation.vm.is_some()
                    && test.next_tick < challenge.data.ticks
            });
            if !more {
                break;
            }
            self.advance_challenge_test_tick();
        }
    }

    /// Executes the next challenge tick: drives the bound input ports with the
    /// expected values, runs the circuit, and records each output port's actual
    /// value against the expected one.
    pub(super) fn advance_challenge_test_tick(&mut self) {
        let Some(challenge) = self.challenge.as_ref() else {
            return;
        };
        let Some(vm) = self.simulation.vm.as_mut() else {
            return;
        };
        let test = &challenge.test;
        let data_ticks = challenge.data.ticks;
        if test.error.is_some() || test.next_tick >= data_ticks {
            return;
        }
        let tick = test.next_tick;
        let input_slots = test.input_slots.clone();
        let output_slots = test.output_slots.clone();
        let input_values = challenge
            .data
            .inputs
            .iter()
            .map(|port| {
                let mask = value_mask(port.scale);
                port.values.get(tick).copied().unwrap_or(0) & mask
            })
            .collect::<Vec<_>>();
        let output_expected = challenge
            .data
            .outputs
            .iter()
            .map(|port| {
                let mask = value_mask(port.scale);
                (mask, port.values.get(tick).copied().unwrap_or(0) & mask)
            })
            .collect::<Vec<_>>();

        vm.begin_tick();
        let input_addresses = vm.input_addresses().to_vec();
        for (port, slot) in input_slots.iter().enumerate() {
            let Some(address) = slot.and_then(|slot| input_addresses.get(slot).copied()) else {
                continue;
            };
            if address >= vm.root_component.memory_size {
                continue;
            }
            vm.root_memory_mut()[address] |= input_values[port];
        }
        vm.execute();

        let output_addresses = vm.output_addresses().to_vec();
        let mut actual = Vec::with_capacity(output_slots.len());
        let mut mismatched = false;
        for (port, slot) in output_slots.iter().enumerate() {
            let (mask, expected) = output_expected[port];
            let value = slot
                .and_then(|slot| output_addresses.get(slot).copied())
                .and_then(|address| vm.root_memory().get(address).copied())
                .map(|value| value & mask)
                .unwrap_or(0);
            mismatched |= value != expected;
            actual.push(value);
        }

        let Some(challenge) = self.challenge.as_mut() else {
            return;
        };
        let test = &mut challenge.test;
        test.mismatched |= mismatched;
        for (port, value) in actual.into_iter().enumerate() {
            test.actual[port].push(value);
        }
        test.next_tick += 1;

        let all_ports = test.input_slots.iter().all(Option::is_some)
            && test.output_slots.iter().all(Option::is_some);
        let passed = test.next_tick == data_ticks && !test.mismatched && all_ports;
        if passed {
            challenge.passed_event = true;
        }
    }

    pub(super) fn show_challenge(&mut self, context: &egui::Context) {
        if self.challenge.is_none() {
            return;
        }
        self.ensure_challenge_test();

        let mut do_step = false;
        let mut do_run = false;
        let mut do_reset = false;
        let mut do_seek = None;

        egui::Window::new("Challenge")
            .default_pos([360.0, 16.0])
            .default_size([320.0, 440.0])
            .resizable(true)
            .show(context, |ui| {
                let Some(challenge) = self.challenge.as_ref() else {
                    return;
                };
                ui.label(&challenge.data.goal);
                ui.separator();
                ui.horizontal(|ui| {
                    do_step = ui.button("Step test").clicked();
                    do_run = ui.button("Run all tests").clicked();
                    do_reset = ui.button("Reset").clicked();
                });

                let data = &challenge.data;
                let test = &challenge.test;
                if let Some(error) = &test.error {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Cannot run: {error}"));
                    return;
                }

                let all_ports = test.input_slots.iter().all(Option::is_some)
                    && test.output_slots.iter().all(Option::is_some);
                let status = if !all_ports {
                    "Place every challenge port to run the test".to_owned()
                } else if test.mismatched {
                    "Failed".to_owned()
                } else if test.next_tick == 0 {
                    "Not run".to_owned()
                } else if test.next_tick < data.ticks {
                    format!("Running {}/{}", test.next_tick, data.ticks)
                } else {
                    "Passed".to_owned()
                };
                ui.label(status);
                ui.separator();

                do_seek = challenge_test_table(ui, data, test);
            });

        if do_reset {
            self.challenge_test_reset();
        }
        if do_step {
            self.challenge_test_step();
        }
        if do_run {
            self.challenge_test_run_all();
        }
        if let Some(tick) = do_seek {
            self.challenge_test_seek(tick);
        }
    }
}
pub(super) fn challenge_port_slots<T: Ord + Copy>(
    ids: impl Iterator<Item = T>,
    count: usize,
    from_index: impl Fn(u128) -> T,
) -> Vec<Option<usize>> {
    let mut ids: Vec<T> = ids.collect();
    ids.sort();
    ids.dedup();
    (0..count)
        .map(|port| ids.binary_search(&from_index(port as u128)).ok())
        .collect()
}

/// Renders the expected/actual table: ticks as rows, ports as columns. Output
/// cells show the actual value (red when wrong) once a tick has run, otherwise
/// the expected value, dimmed.
/// Renders the challenge test table. The row whose inputs are currently driven
/// onto the wires (the last executed tick) is highlighted. Returns the tick of a
/// row the user clicked, if any, so the caller can seek the test to it.
pub(super) fn challenge_test_table(
    ui: &mut egui::Ui,
    data: &Challenge,
    test: &ChallengeTest,
) -> Option<usize> {
    const CELL_WIDTH: f32 = 52.0;
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let error_color = ui.visuals().error_fg_color;
    let weak_color = ui.visuals().weak_text_color();
    let active_tick = test.next_tick.checked_sub(1);
    let mut clicked = None;

    let cell = |ui: &mut egui::Ui, text: String, color: Option<egui::Color32>, strong: bool| {
        let mut rich = egui::RichText::new(text).monospace();
        if strong {
            rich = rich.strong();
        }
        if let Some(color) = color {
            rich = rich.color(color);
        }
        ui.add_sized(
            [CELL_WIDTH, row_height],
            egui::Label::new(rich).wrap_mode(egui::TextWrapMode::Extend),
        );
    };

    ui.horizontal(|ui| {
        cell(ui, "Tick".to_owned(), None, true);
        for port in &data.inputs {
            cell(ui, port.label.to_owned(), None, true);
        }
        for port in &data.outputs {
            cell(ui, port.label.to_owned(), None, true);
        }
    });

    let highlight_color = ui.visuals().selection.bg_fill;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, data.ticks, |ui, range| {
            for tick in range {
                // Reserve a shape slot so the highlight paints behind the cells.
                let background = ui.painter().add(egui::Shape::Noop);
                let row = ui
                    .horizontal(|ui| {
                        cell(ui, tick.to_string(), None, false);
                        for port in &data.inputs {
                            let value = port.values.get(tick).copied().unwrap_or(0);
                            cell(ui, value.to_string(), None, false);
                        }
                        for (index, port) in data.outputs.iter().enumerate() {
                            let expected = port.values.get(tick).copied().unwrap_or(0);
                            if tick < test.next_tick {
                                let actual = test
                                    .actual
                                    .get(index)
                                    .and_then(|values| values.get(tick))
                                    .copied()
                                    .unwrap_or(expected);
                                let color = (actual != expected).then_some(error_color);
                                cell(ui, actual.to_string(), color, false);
                            } else {
                                cell(ui, expected.to_string(), Some(weak_color), false);
                            }
                        }
                    })
                    .response
                    .interact(egui::Sense::click());
                if active_tick == Some(tick) {
                    ui.painter().set(
                        background,
                        egui::Shape::rect_filled(row.rect, 2.0, highlight_color),
                    );
                }
                if row.clicked() {
                    clicked = Some(tick);
                }
                if row.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });

    clicked
}
