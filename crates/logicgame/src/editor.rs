use std::collections::{BTreeMap, BTreeSet};

use eframe::egui::{self, PointerButton};
use logicgame::execution::{GenerationError, Instruction, Vm};
use logicgame::grid::{
    CircuitGraph, Component, ComponentId, ComponentKind, ConnectionSlot, GraphNode, LogicGrid,
    Point, Rotation, Scale, ValidationError, Wire,
};

use crate::renderer::{DrawTriangle, GridCallback, RenderFrame};

const MIN_ZOOM: f32 = 4.0;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const GRAPH_NODE_SIZE: egui::Vec2 = egui::vec2(150.0, 48.0);
const GRAPH_COLUMN_GAP: f32 = 70.0;
const GRAPH_ROW_GAP: f32 = 18.0;
const GRAPH_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Select,
    Wire,
    Not,
    Led,
    Storage,
    ConfigureStorage,
}

impl ToolKind {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Not => "NOT gate",
            Self::Led => "LED",
            Self::Storage => "Storage",
            Self::ConfigureStorage => "Configure storage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tool {
    kind: ToolKind,
    scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    center: [f32; 2],
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            zoom: DEFAULT_ZOOM,
        }
    }
}

impl Camera {
    fn screen_to_world(self, screen: egui::Pos2, rect: egui::Rect) -> [f32; 2] {
        let relative = screen - rect.center();
        [
            self.center[0] + relative.x / self.zoom,
            self.center[1] + relative.y / self.zoom,
        ]
    }

    fn zoom_around(&mut self, screen: egui::Pos2, rect: egui::Rect, factor: f32) {
        let before = self.screen_to_world(screen, rect);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(screen, rect);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Gesture {
    Wire {
        start: Point,
    },
    Not {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Led {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Storage {
        anchor: Point,
        drag_start: [f32; 2],
    },
    SelectBox {
        start: [f32; 2],
        additive: bool,
    },
    MoveSelection {
        start: [f32; 2],
        scale: Scale,
        components: Vec<(ComponentId, Point)>,
        wires: Vec<SelectedWire>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DebugEntity {
    Component(ComponentId),
    Wire(Wire),
    WireEndpoint(WireEndpoint),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum WireEnd {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WireEndpoint {
    wire: Wire,
    end: WireEnd,
}

impl WireEndpoint {
    fn point(self) -> Point {
        match self.end {
            WireEnd::Start => self.wire.start,
            WireEnd::End => self.wire.end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedWire {
    wire: Wire,
    start: bool,
    end: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selection {
    components: BTreeSet<ComponentId>,
    wire_endpoints: BTreeSet<WireEndpoint>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct GraphHover {
    components: BTreeSet<ComponentId>,
    connections: BTreeSet<(ComponentId, ConnectionSlot)>,
    wires: BTreeSet<Wire>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SimulationSnapshot {
    components: Vec<Component>,
    wires: Vec<Wire>,
    graph: CircuitGraph,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Simulation {
    snapshot: Option<SimulationSnapshot>,
    vm: Option<Vm>,
    error: Option<GenerationError>,
    steps: u64,
    next_instruction: usize,
    tick_in_progress: bool,
}

impl GraphHover {
    fn include_node(&mut self, node: &GraphNode) {
        match node {
            GraphNode::Component { component } => {
                self.components.insert(*component);
            }
            GraphNode::WireNet { wires } => {
                self.wires.extend(wires.iter().copied());
            }
            GraphNode::Connection {
                component,
                slot,
                direction,
                side,
                start,
                end,
            } => {
                self.connections.insert((
                    *component,
                    ConnectionSlot {
                        id: *slot,
                        direction: *direction,
                        side: *side,
                        start: *start,
                        end: *end,
                    },
                ));
            }
        }
    }
}

impl Selection {
    fn is_empty(&self) -> bool {
        self.components.is_empty() && self.wire_endpoints.is_empty()
    }

    fn clear(&mut self) {
        self.components.clear();
        self.wire_endpoints.clear();
    }

    fn contains(&self, entity: DebugEntity) -> bool {
        match entity {
            DebugEntity::Component(id) => self.components.contains(&id),
            DebugEntity::Wire(wire) => self
                .wire_endpoints
                .iter()
                .any(|endpoint| endpoint.wire == wire),
            DebugEntity::WireEndpoint(endpoint) => self.wire_endpoints.contains(&endpoint),
        }
    }

    fn insert(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                self.components.insert(id);
            }
            DebugEntity::Wire(wire) => {
                self.wire_endpoints.extend([
                    WireEndpoint {
                        wire,
                        end: WireEnd::Start,
                    },
                    WireEndpoint {
                        wire,
                        end: WireEnd::End,
                    },
                ]);
            }
            DebugEntity::WireEndpoint(endpoint) => {
                self.wire_endpoints.insert(endpoint);
            }
        }
    }

    fn toggle(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                if !self.components.remove(&id) {
                    self.components.insert(id);
                }
            }
            DebugEntity::Wire(wire) => {
                let endpoints = [
                    WireEndpoint {
                        wire,
                        end: WireEnd::Start,
                    },
                    WireEndpoint {
                        wire,
                        end: WireEnd::End,
                    },
                ];
                if endpoints
                    .iter()
                    .any(|endpoint| self.wire_endpoints.contains(endpoint))
                {
                    for endpoint in endpoints {
                        self.wire_endpoints.remove(&endpoint);
                    }
                } else {
                    self.wire_endpoints.extend(endpoints);
                }
            }
            DebugEntity::WireEndpoint(endpoint) => {
                if !self.wire_endpoints.remove(&endpoint) {
                    self.wire_endpoints.insert(endpoint);
                }
            }
        }
    }

    fn selected_wires(&self) -> Vec<SelectedWire> {
        let mut wires = BTreeMap::<Wire, SelectedWire>::new();
        for endpoint in &self.wire_endpoints {
            let selected = wires.entry(endpoint.wire).or_insert(SelectedWire {
                wire: endpoint.wire,
                start: false,
                end: false,
            });
            match endpoint.end {
                WireEnd::Start => selected.start = true,
                WireEnd::End => selected.end = true,
            }
        }
        wires.into_values().collect()
    }
}

pub struct LogicEditor {
    grid: LogicGrid,
    tool: Tool,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
    configured_storage: Option<ComponentId>,
    simulation: Simulation,
}

impl Default for LogicEditor {
    fn default() -> Self {
        Self {
            grid: LogicGrid::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
            },
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
            configured_storage: None,
            simulation: Simulation::default(),
        }
    }
}

impl LogicEditor {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let canvas = egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            self.handle_canvas_input(&response);

            let pointer_world = response
                .hovered()
                .then(|| context.pointer_hover_pos())
                .flatten()
                .map(|position| self.camera.screen_to_world(position, response.rect));
            (response, painter, pointer_world)
        });

        egui::Window::new("Tools")
            .default_pos([16.0, 16.0])
            .resizable(false)
            .show(&context, |ui| {
                for kind in [
                    ToolKind::Select,
                    ToolKind::Wire,
                    ToolKind::Not,
                    ToolKind::Led,
                    ToolKind::Storage,
                    ToolKind::ConfigureStorage,
                ] {
                    if ui
                        .selectable_label(self.tool.kind == kind, kind.label())
                        .clicked()
                    {
                        self.tool.kind = kind;
                        self.gesture = None;
                        self.configured_storage = None;
                        if kind != ToolKind::Select {
                            self.selection.clear();
                        }
                    }
                }

                ui.separator();
                ui.label("Scale");
                egui::ComboBox::from_id_salt("logic-tool-scale")
                    .selected_text(format!("{}x", self.tool.scale.get()))
                    .show_ui(ui, |ui| {
                        for value in SCALES {
                            let scale = Scale::new(value).expect("tool scale is valid");
                            ui.selectable_value(&mut self.tool.scale, scale, format!("{value}x"));
                        }
                    });
                ui.separator();
                ui.small("Middle drag: pan");
                ui.small("Wheel: zoom");
                ui.small("Shift: add/remove selection");
                ui.small("Delete: remove selection");
                ui.small("Esc: cancel");
            });

        self.show_storage_configuration(&context);
        self.show_simulation(&context);

        let hovered_square = canvas
            .inner
            .2
            .map(|pointer| snap_point(pointer, self.tool.scale));
        let hovered_entity = self.show_grid_debugger(&context, hovered_square);
        let graph_hover = self.show_generated_graph(&context);
        let frame = self.render_frame(
            canvas.inner.0.rect,
            canvas.inner.2,
            hovered_entity,
            &graph_hover,
        );
        canvas
            .inner
            .1
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                canvas.inner.0.rect,
                GridCallback { frame },
            ));
        if let (Some(pointer), Some(Gesture::SelectBox { start, .. })) =
            (canvas.inner.2, self.gesture.as_ref())
        {
            let start = world_to_screen(*start, self.camera, canvas.inner.0.rect);
            let end = world_to_screen(pointer, self.camera, canvas.inner.0.rect);
            let selection_rect = egui::Rect::from_two_pos(start, end);
            canvas.inner.1.rect_filled(
                selection_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(66, 153, 225, 28),
            );
            canvas.inner.1.rect_stroke(
                selection_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 180, 255)),
                egui::StrokeKind::Inside,
            );
        }

        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.gesture = None;
        }

        context.request_repaint();
    }

    fn show_simulation(&mut self, context: &egui::Context) {
        egui::Window::new("Simulation")
            .default_pos([16.0, 390.0])
            .default_width(300.0)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Run step").clicked() {
                        self.run_simulation_step();
                    }
                    if ui.button("Step instruction").clicked() {
                        self.run_simulation_instruction();
                    }
                    if ui.button("Restart").clicked() {
                        self.restart_simulation();
                    }
                });

                ui.separator();
                if let Some(error) = &self.simulation.error {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Cannot run: {error:?}"),
                    );
                    return;
                }

                let Some(vm) = &self.simulation.vm else {
                    ui.weak("Run a step to compile and execute the circuit.");
                    return;
                };
                let Some(snapshot) = &self.simulation.snapshot else {
                    return;
                };

                egui::Grid::new("logic-simulation-summary")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Steps");
                        ui.monospace(self.simulation.steps.to_string());
                        ui.end_row();
                        ui.label("Instructions");
                        ui.monospace(vm.instructions.len().to_string());
                        ui.end_row();
                        ui.label("Next instruction");
                        if self.simulation.next_instruction < vm.instructions.len() {
                            ui.monospace(format!(
                                "{} / {}",
                                self.simulation.next_instruction + 1,
                                vm.instructions.len()
                            ));
                        } else {
                            if vm.instructions.is_empty() {
                                ui.weak("none");
                            } else {
                                ui.weak("tick complete");
                            }
                        }
                        ui.end_row();
                    });

                ui.separator();
                ui.strong("Instructions");
                if vm.instructions.is_empty() {
                    ui.weak("No instructions");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("logic-simulation-instructions")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, instruction) in vm.instructions.iter().enumerate() {
                                let next = self.simulation.next_instruction == index
                                    || index == 0
                                        && self.simulation.next_instruction
                                            >= vm.instructions.len();
                                let response = ui.selectable_label(
                                    next,
                                    egui::RichText::new(format!(
                                        "{index:03}  {}",
                                        format_instruction(instruction)
                                    ))
                                    .monospace(),
                                );
                                if next {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                            }
                        });
                }

                ui.separator();
                ui.strong("Wire groups");
                if vm.memory.is_empty() {
                    ui.weak("No connected wire groups");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("logic-simulation-wires")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (address, value) in vm.memory.iter().copied().enumerate() {
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

    fn run_simulation_step(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        while self.simulation.tick_in_progress {
            self.execute_next_simulation_instruction();
        }
    }

    fn run_simulation_instruction(&mut self) {
        if !self.prepare_simulation() || !self.begin_simulation_tick() {
            return;
        }
        self.execute_next_simulation_instruction();
    }

    fn restart_simulation(&mut self) {
        let snapshot = self.simulation_snapshot();
        self.compile_simulation(snapshot);
    }

    fn compile_simulation(&mut self, snapshot: SimulationSnapshot) {
        match Vm::from_graph(&self.grid, &snapshot.graph) {
            Ok(vm) => {
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    vm: Some(vm),
                    error: None,
                    steps: 0,
                    next_instruction: 0,
                    tick_in_progress: false,
                };
            }
            Err(error) => {
                self.simulation = Simulation {
                    snapshot: Some(snapshot),
                    vm: None,
                    error: Some(error),
                    steps: 0,
                    next_instruction: 0,
                    tick_in_progress: false,
                };
            }
        }
    }

    fn prepare_simulation(&mut self) -> bool {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
        }
        self.simulation.vm.is_some()
    }

    fn begin_simulation_tick(&mut self) -> bool {
        if self.simulation.tick_in_progress {
            return true;
        }
        let Some(vm) = &mut self.simulation.vm else {
            return false;
        };
        vm.memory.fill(0);
        self.simulation.next_instruction = 0;
        if vm.instructions.is_empty() {
            self.simulation.steps += 1;
            return false;
        }
        self.simulation.tick_in_progress = true;
        true
    }

    fn execute_next_simulation_instruction(&mut self) {
        let Some(vm) = &mut self.simulation.vm else {
            return;
        };
        let instruction = self.simulation.next_instruction;
        vm.execute_instruction(instruction);
        self.simulation.next_instruction += 1;
        if self.simulation.next_instruction == vm.instructions.len() {
            self.simulation.steps += 1;
            self.simulation.tick_in_progress = false;
        }
    }

    fn simulation_snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            components: self.grid.components().cloned().collect(),
            wires: self.grid.wires().to_vec(),
            graph: self.grid.generate_graph(),
        }
    }

    fn show_storage_configuration(&mut self, context: &egui::Context) {
        let Some(id) = self.configured_storage else {
            return;
        };
        let Some((scale, value)) = self.grid.component(id).and_then(|component| {
            if let ComponentKind::Storage { scale, value } = &component.kind {
                Some((*scale, *value))
            } else {
                None
            }
        }) else {
            self.configured_storage = None;
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Configure storage #{}", id.0))
            .open(&mut open)
            .default_width(180.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(480.0)
                    .show(ui, |ui| {
                        for bit in storage_bit_indices(scale) {
                            let state = (value >> bit) & 1;
                            if ui.button(format!("Bit {bit}: {state}")).clicked() {
                                self.grid.toggle_storage_bit(id, bit);
                            }
                        }
                    });
            });
        if !open {
            self.configured_storage = None;
        }
    }

    fn show_grid_debugger(
        &self,
        context: &egui::Context,
        hovered_square: Option<Point>,
    ) -> Option<DebugEntity> {
        let errors = self.grid.validate();
        let mut hovered_entity = None;

        egui::Window::new("Grid Debugger")
            .default_pos([700.0, 16.0])
            .default_width(240.0)
            .show(context, |ui| {
                egui::Grid::new("logic-grid-debug-summary")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Components");
                        ui.monospace(self.grid.components().count().to_string());
                        ui.end_row();
                        ui.label("Wires");
                        ui.monospace(self.grid.wires().len().to_string());
                        ui.end_row();
                        ui.label("Validation errors");
                        ui.monospace(errors.len().to_string());
                        ui.end_row();
                        ui.label("Hovered square");
                        match hovered_square {
                            Some(point) => {
                                ui.monospace(format!("({}, {})", point.x, point.y));
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();
                    });

                ui.separator();
                ui.strong("Validation errors");
                if errors.is_empty() {
                    ui.weak("No validation errors");
                } else {
                    egui::ScrollArea::both()
                        .id_salt("logic-grid-validation-errors")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, error) in errors.iter().enumerate() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("#{index} {error:#?}"))
                                            .monospace(),
                                    )
                                    .selectable(true),
                                );
                            }
                        });
                }

                ui.separator();
                ui.strong("Entities");
                if self.grid.components().next().is_none() && self.grid.wires().is_empty() {
                    ui.weak("No entities");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            ui.label("Components");
                            for component in self.grid.components() {
                                let response = egui::CollapsingHeader::new(format!(
                                    "#{} {}",
                                    component.id.0,
                                    component_kind_name(&component.kind)
                                ))
                                .id_salt(("logic-grid-entity", component.id.0))
                                .show(ui, |ui| {
                                    egui::Grid::new(("logic-grid-entity-details", component.id.0))
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            ui.label("Position");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                component.position.x, component.position.y
                                            ));
                                            ui.end_row();
                                            ui.label("Rotation");
                                            ui.monospace(format!("{:?}", component.rotation));
                                            ui.end_row();
                                            ui.label("Kind");
                                            ui.monospace(format!("{:?}", component.kind));
                                            ui.end_row();
                                            ui.label("Size");
                                            match component.size() {
                                                Some(size) => {
                                                    ui.monospace(format!(
                                                        "{} x {}",
                                                        size.width, size.height
                                                    ));
                                                }
                                                None => {
                                                    ui.weak("overflow");
                                                }
                                            }
                                            ui.end_row();
                                        });
                                });

                                let body_hovered =
                                    response.body_response.is_some_and(|body| body.hovered());
                                if response.header_response.hovered() || body_hovered {
                                    hovered_entity = Some(DebugEntity::Component(component.id));
                                }
                            }

                            ui.separator();
                            ui.label("Wire segments");
                            for (index, wire) in self.grid.wires().iter().copied().enumerate() {
                                let response = egui::CollapsingHeader::new(format!(
                                    "#{index} {:?}",
                                    wire.orientation()
                                ))
                                .id_salt(("logic-grid-wire", index))
                                .show(ui, |ui| {
                                    egui::Grid::new(("logic-grid-wire-details", index))
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            ui.label("Start");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                wire.start.x, wire.start.y
                                            ));
                                            ui.end_row();
                                            ui.label("End");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                wire.end.x, wire.end.y
                                            ));
                                            ui.end_row();
                                            ui.label("Orientation");
                                            ui.monospace(format!("{:?}", wire.orientation()));
                                            ui.end_row();
                                            ui.label("Length");
                                            ui.monospace(wire.length().to_string());
                                            ui.end_row();
                                            ui.label("Scale");
                                            ui.monospace(format!("{}x", wire.scale.get()));
                                            ui.end_row();
                                        });
                                });

                                let body_hovered =
                                    response.body_response.is_some_and(|body| body.hovered());
                                if response.header_response.hovered() || body_hovered {
                                    hovered_entity = Some(DebugEntity::Wire(wire));
                                }
                            }
                        });
                }
            });

        hovered_entity
    }

    fn show_generated_graph(&self, context: &egui::Context) -> GraphHover {
        let graph = self.grid.generate_graph();
        let mut hover = GraphHover::default();

        egui::Window::new("Generated Graph")
            .default_pos([360.0, 16.0])
            .default_size([560.0, 400.0])
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Nodes");
                    ui.monospace(graph.nodes.len().to_string());
                    ui.separator();
                    ui.label("Edges");
                    ui.monospace(graph.edges.len().to_string());
                });
                ui.separator();

                if graph.nodes.is_empty() {
                    ui.weak("The generated graph is empty.");
                    return;
                }

                egui::ScrollArea::both().show(ui, |ui| {
                    self.paint_generated_graph(ui, &graph, &mut hover);
                });
            });

        hover
    }

    fn paint_generated_graph(
        &self,
        ui: &mut egui::Ui,
        graph: &CircuitGraph,
        hover: &mut GraphHover,
    ) {
        let (positions, size) = graph_layout(graph);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        let origin = response.rect.min.to_vec2();

        for edge in &graph.edges {
            let first = positions[edge.first.0] + origin;
            let second = positions[edge.second.0] + origin;
            painter.line_segment(
                [first, second],
                egui::Stroke::new(2.0, ui.visuals().widgets.noninteractive.fg_stroke.color),
            );
        }

        for (index, node) in graph.nodes.iter().enumerate() {
            let center = positions[index] + origin;
            let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE);
            let (fill, title, detail) = graph_node_display(node);
            painter.rect_filled(rect, 6.0, fill);
            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );
            painter.text(
                center - egui::vec2(0.0, 9.0),
                egui::Align2::CENTER_CENTER,
                format!("#{index} {title}"),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            painter.text(
                center + egui::vec2(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                detail,
                egui::FontId::monospace(11.0),
                egui::Color32::from_gray(225),
            );

            let node_response = ui.interact(
                rect,
                ui.id().with(("generated-graph-node", index)),
                egui::Sense::hover(),
            );
            if node_response.hovered() {
                hover.include_node(node);
            }
        }
    }

    fn handle_canvas_input(&mut self, response: &egui::Response) {
        if self.tool.kind == ToolKind::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.delete_selection();
        }

        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };

        if response.hovered() {
            let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera
                    .zoom_around(pointer, response.rect, (scroll * 0.002).exp());
            }
        }

        let middle_down = response
            .ctx
            .input(|input| input.pointer.button_down(PointerButton::Middle));
        if middle_down && response.hovered() {
            let delta = response.ctx.input(|input| input.pointer.delta());
            self.camera.center[0] -= delta.x / self.camera.zoom;
            self.camera.center[1] -= delta.y / self.camera.zoom;
            return;
        }

        let world = self.camera.screen_to_world(pointer, response.rect);
        let snapped = snap_point(world, self.tool.scale);

        if response.clicked_by(PointerButton::Secondary) && self.tool.kind == ToolKind::Wire {
            if let Some(wire) =
                nearest_wire(self.grid.wires(), world, WIRE_HIT_RADIUS / self.camera.zoom)
            {
                self.grid.remove_wire(wire);
            }
        }

        let primary_pressed = response
            .ctx
            .input(|input| input.pointer.button_pressed(PointerButton::Primary));
        if primary_pressed && response.hovered() {
            self.gesture = match self.tool.kind {
                ToolKind::Select => {
                    let additive = response.ctx.input(|input| input.modifiers.shift);
                    let hit = self.entity_at(world);
                    match hit {
                        Some(entity) if additive => {
                            self.selection.toggle(entity);
                            None
                        }
                        Some(entity) => {
                            if !self.selection.contains(entity) {
                                self.selection.clear();
                                self.selection.insert(entity);
                            }
                            self.move_gesture(world)
                        }
                        None => Some(Gesture::SelectBox {
                            start: world,
                            additive,
                        }),
                    }
                }
                ToolKind::Wire => Some(Gesture::Wire { start: snapped }),
                ToolKind::Not => Some(Gesture::Not {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Led => Some(Gesture::Led {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Storage => Some(Gesture::Storage {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::ConfigureStorage => {
                    if let Some(DebugEntity::Component(id)) = self.entity_at(world) {
                        if let Some(ComponentKind::Storage { scale, .. }) =
                            self.grid.component(id).map(|component| &component.kind)
                        {
                            if *scale == Scale::ONE {
                                self.grid.toggle_storage_bit(id, 0);
                            } else {
                                self.configured_storage = Some(id);
                            }
                        }
                    }
                    None
                }
            };
        }

        let primary_released = response
            .ctx
            .input(|input| input.pointer.button_released(PointerButton::Primary));
        if primary_released {
            match self.gesture.take() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(start, snapped, self.tool.scale) {
                        self.edit_grid_preserving_edge_attachments(|grid| {
                            grid.add_wire(wire);
                        });
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let scale = self.tool.scale;
                        self.edit_grid_preserving_edge_attachments(|grid| {
                            grid.add_component(
                                oriented_component_position(anchor, rotation, scale),
                                rotation,
                                ComponentKind::Not { scale },
                            );
                        });
                    }
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        self.edit_grid_preserving_edge_attachments(|grid| {
                            grid.add_component(
                                oriented_component_position(anchor, rotation, Scale::ONE),
                                rotation,
                                ComponentKind::Led,
                            );
                        });
                    }
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let scale = self.tool.scale;
                        self.edit_grid_preserving_edge_attachments(|grid| {
                            grid.add_component(
                                oriented_component_position(anchor, rotation, scale),
                                rotation,
                                ComponentKind::Storage { scale, value: 0 },
                            );
                        });
                    }
                }
                Some(Gesture::SelectBox { start, additive }) => {
                    if !additive {
                        self.selection.clear();
                    }
                    self.select_in_rect(start, world);
                }
                Some(Gesture::MoveSelection {
                    start,
                    scale,
                    components,
                    wires,
                }) => {
                    let delta = snapped_delta(start, world, scale);
                    self.apply_move(&components, &wires, delta);
                }
                None => {}
            }
        }
    }

    fn entity_at(&self, point: [f32; 2]) -> Option<DebugEntity> {
        self.grid
            .components()
            .filter(|component| component_contains(component, point))
            .map(|component| DebugEntity::Component(component.id))
            .max_by_key(|entity| match entity {
                DebugEntity::Component(id) => *id,
                DebugEntity::Wire(_) | DebugEntity::WireEndpoint(_) => unreachable!(),
            })
            .or_else(|| {
                nearest_wire_endpoint(self.grid.wires(), point, WIRE_HIT_RADIUS / self.camera.zoom)
                    .map(DebugEntity::WireEndpoint)
            })
    }

    fn move_gesture(&self, start: [f32; 2]) -> Option<Gesture> {
        if self.selection.is_empty() {
            return None;
        }
        let components: Vec<_> = self
            .selection
            .components
            .iter()
            .filter_map(|id| {
                self.grid
                    .component(*id)
                    .map(|component| (*id, component.position))
            })
            .collect();
        let wires = self.selection.selected_wires();
        let scale = components
            .iter()
            .filter_map(|(id, _)| self.grid.component(*id))
            .map(|component| component.kind.snap())
            .chain(wires.iter().map(|wire| wire.wire.scale))
            .max()
            .unwrap_or(Scale::ONE);
        Some(Gesture::MoveSelection {
            start,
            scale,
            components,
            wires,
        })
    }

    fn select_in_rect(&mut self, start: [f32; 2], end: [f32; 2]) {
        let rect = WorldRect::from_points(start, end);
        self.selection.components.extend(
            self.grid
                .components()
                .filter(|component| component_intersects(component, rect))
                .map(|component| component.id),
        );
        for wire in self.grid.wires().iter().copied() {
            for end in [WireEnd::Start, WireEnd::End] {
                let endpoint = WireEndpoint { wire, end };
                if point_cell_intersects(endpoint.point(), wire.scale, rect) {
                    self.selection.wire_endpoints.insert(endpoint);
                }
            }
        }
    }

    fn apply_move(
        &mut self,
        components: &[(ComponentId, Point)],
        wires: &[SelectedWire],
        delta: Point,
    ) {
        if delta == Point::new(0, 0) {
            return;
        }
        let moved_components: Option<Vec<_>> = components
            .iter()
            .map(|(id, position)| translate_point(*position, delta).map(|point| (*id, point)))
            .collect();
        let moved_wires: Vec<_> = wires
            .iter()
            .filter_map(|wire| move_selected_wire(*wire, delta))
            .collect();
        let Some(moved_components) = moved_components else {
            return;
        };

        self.edit_grid_preserving_edge_attachments(|grid| {
            for wire in wires {
                grid.remove_wire(wire.wire);
            }
            for (id, position) in moved_components {
                grid.set_component_position(id, position);
            }
            for wire in &moved_wires {
                grid.add_wire(*wire);
            }
        });

        let selected_points: BTreeSet<_> = wires
            .iter()
            .filter_map(|wire| {
                move_selected_wire(*wire, delta).map(|moved| {
                    let mut points = Vec::new();
                    if wire.start {
                        points.push((
                            translate_point(wire.wire.start, delta).unwrap(),
                            moved.scale,
                        ));
                    }
                    if wire.end {
                        points.push((translate_point(wire.wire.end, delta).unwrap(), moved.scale));
                    }
                    points
                })
            })
            .flatten()
            .collect();
        self.selection.wire_endpoints = self
            .grid
            .wires()
            .iter()
            .copied()
            .flat_map(|wire| {
                [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end })
            })
            .filter(|endpoint| selected_points.contains(&(endpoint.point(), endpoint.wire.scale)))
            .collect();
    }

    fn edit_grid_preserving_edge_attachments(&mut self, edit: impl FnOnce(&mut LogicGrid)) {
        let Some(old_bounds) = BoardBounds::from_grid(&self.grid) else {
            edit(&mut self.grid);
            return;
        };
        let attachments = BoardBounds::edge_attachments(&self.grid, old_bounds);
        edit(&mut self.grid);
        if attachments.is_empty() {
            return;
        }

        let ignored_endpoints: BTreeSet<_> = attachments
            .iter()
            .map(|attachment| (attachment.point, attachment.wire.scale))
            .collect();
        let Some(new_bounds) =
            BoardBounds::from_grid_ignoring_endpoints(&self.grid, &ignored_endpoints)
        else {
            return;
        };

        let mut replacements = BTreeMap::<Wire, Wire>::new();
        for attachment in attachments {
            if !self.grid.wires().contains(&attachment.wire) {
                continue;
            }
            let wire = replacements
                .get(&attachment.wire)
                .copied()
                .unwrap_or(attachment.wire);
            if let Some(wire) = attachment.extend(wire, old_bounds, new_bounds) {
                replacements.insert(attachment.wire, wire);
            }
        }
        for (original, replacement) in replacements {
            self.grid.remove_wire(original);
            self.grid.add_wire(replacement);
        }
    }

    fn delete_selection(&mut self) {
        for id in std::mem::take(&mut self.selection.components) {
            self.grid.remove_component(id);
        }
        let wires: BTreeSet<_> = std::mem::take(&mut self.selection.wire_endpoints)
            .into_iter()
            .map(|endpoint| endpoint.wire)
            .collect();
        for wire in wires {
            self.grid.remove_wire(wire);
        }
        self.gesture = None;
    }

    fn render_frame(
        &self,
        rect: egui::Rect,
        pointer_world: Option<[f32; 2]>,
        hovered_entity: Option<DebugEntity>,
        graph_hover: &GraphHover,
    ) -> RenderFrame {
        let errors = self.grid.validate();
        let mut bad_wires = BTreeSet::new();
        let mut bad_components = BTreeSet::new();
        for error in errors {
            match error {
                ValidationError::WireComponentIntersection { wire, component } => {
                    bad_wires.insert(wire);
                    bad_components.insert(component);
                }
                ValidationError::WireNotSnapped { wire }
                | ValidationError::WireOverflow { wire } => {
                    bad_wires.insert(wire);
                }
                ValidationError::ComponentOverflow { component }
                | ValidationError::ComponentNotSnapped { component, .. } => {
                    bad_components.insert(component);
                }
                ValidationError::ComponentOverlap { first, second } => {
                    bad_components.insert(first);
                    bad_components.insert(second);
                }
            }
        }

        let mut component_triangles = Vec::new();
        let mut wire_triangles = Vec::new();
        let mut preview_components = Vec::new();
        let mut preview_wires = Vec::new();
        for component in self.grid.components() {
            component_triangles.extend(DrawTriangle::component(
                component,
                bad_components.contains(&component.id),
            ));
            if hovered_entity == Some(DebugEntity::Component(component.id))
                || graph_hover.components.contains(&component.id)
                || self.selection.components.contains(&component.id)
            {
                component_triangles.extend(DrawTriangle::component_highlight(component));
            }
            for (_, connection) in graph_hover
                .connections
                .iter()
                .filter(|(id, _)| *id == component.id)
            {
                component_triangles
                    .extend(DrawTriangle::connection_highlight(component, *connection));
            }
        }
        for wire in self.grid.wires() {
            wire_triangles.extend(DrawTriangle::wire(
                *wire,
                if hovered_entity == Some(DebugEntity::Wire(*wire))
                    || graph_hover.wires.contains(wire)
                {
                    DrawTriangle::HIGHLIGHT_COLOR
                } else if bad_wires.contains(wire) {
                    DrawTriangle::ERROR_COLOR
                } else {
                    DrawTriangle::WIRE_COLOR
                },
            ));
            for end in [WireEnd::Start, WireEnd::End] {
                let endpoint = WireEndpoint { wire: *wire, end };
                if hovered_entity == Some(DebugEntity::WireEndpoint(endpoint))
                    || self.selection.wire_endpoints.contains(&endpoint)
                {
                    wire_triangles.extend(DrawTriangle::wire_endpoint_highlight(
                        *wire,
                        endpoint.point(),
                    ));
                }
            }
        }

        if let Some(pointer) = pointer_world {
            let snapped = snap_point(pointer, self.tool.scale);
            match self.gesture.as_ref() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(*start, snapped, self.tool.scale) {
                        preview_wires.push(wire);
                        wire_triangles
                            .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: oriented_component_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                            ),
                            rotation,
                            kind: ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        };
                        preview_components.push(component.clone());
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: oriented_component_position(*anchor, rotation, Scale::ONE),
                            rotation,
                            kind: ComponentKind::Led,
                        };
                        preview_components.push(component.clone());
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: oriented_component_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                            ),
                            rotation,
                            kind: ComponentKind::Storage {
                                scale: self.tool.scale,
                                value: 0,
                            },
                        };
                        preview_components.push(component.clone());
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::MoveSelection {
                    start,
                    scale,
                    components,
                    wires,
                }) => {
                    let delta = snapped_delta(*start, pointer, *scale);
                    for (id, position) in components {
                        let Some(position) = translate_point(*position, delta) else {
                            continue;
                        };
                        let Some(original) = self.grid.component(*id) else {
                            continue;
                        };
                        let mut component = original.clone();
                        component.position = position;
                        preview_components.push(component.clone());
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                    for wire in wires {
                        if let Some(wire) = move_selected_wire(*wire, delta) {
                            preview_wires.push(wire);
                            wire_triangles
                                .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                        }
                    }
                }
                Some(Gesture::SelectBox { .. }) => {}
                None => {}
            }
        }
        wire_triangles.extend(component_triangles);
        let board_bounds =
            BoardBounds::from_grid_and_previews(&self.grid, &preview_components, &preview_wires)
                .unwrap_or_default();

        RenderFrame {
            viewport_size: [rect.width(), rect.height()],
            camera_center: self.camera.center,
            zoom: self.camera.zoom,
            grid_scale: self.tool.scale.get() as f32,
            board_bounds: board_bounds.as_f32(),
            triangles: wire_triangles,
        }
    }
}

fn component_kind_name(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Not { .. } => "NOT gate",
        ComponentKind::Led => "LED",
        ComponentKind::Storage { .. } => "Storage",
        ComponentKind::Subcomponent { .. } => "Subcomponent",
    }
}

fn storage_bit_indices(scale: Scale) -> Vec<u32> {
    (0..scale.get() as u32).rev().collect()
}

fn simulation_value_row(ui: &mut egui::Ui, label: String, value: u64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.monospace(format!("0x{value:016x}"));
        ui.weak(format!("({value})"));
    });
}

fn format_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Call {
            component,
            inputs,
            outputs,
        } => format!("CALL {component} {inputs:?} -> {outputs:?}"),
        Instruction::Not { input, output } => format!("NOT m{input} -> m{output}"),
        Instruction::ReadStorage { storage, output } => {
            format!("READ s{storage} -> m{output}")
        }
        Instruction::SaveStorage { storage, input } => {
            format!("SAVE m{input} -> s{storage}")
        }
    }
}

fn snap_coordinate(value: f32, scale: Scale) -> i64 {
    let scale = scale.get();
    (value / scale as f32).floor() as i64 * scale
}

fn snap_point(point: [f32; 2], scale: Scale) -> Point {
    Point::new(
        snap_coordinate(point[0], scale),
        snap_coordinate(point[1], scale),
    )
}

fn snapped_delta(start: [f32; 2], end: [f32; 2], scale: Scale) -> Point {
    let scale = scale.get() as f32;
    Point::new(
        ((end[0] - start[0]) / scale).round() as i64 * scale as i64,
        ((end[1] - start[1]) / scale).round() as i64 * scale as i64,
    )
}

fn translate_point(point: Point, delta: Point) -> Option<Point> {
    Some(Point::new(
        point.x.checked_add(delta.x)?,
        point.y.checked_add(delta.y)?,
    ))
}

fn move_selected_wire(selected: SelectedWire, delta: Point) -> Option<Wire> {
    let start = if selected.start {
        translate_point(selected.wire.start, delta)?
    } else {
        selected.wire.start
    };
    let end = if selected.end {
        translate_point(selected.wire.end, delta)?
    } else {
        selected.wire.end
    };
    Wire::new(start, end, selected.wire.scale).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoardBounds {
    min: Point,
    max: Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoardSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EdgeAttachment {
    wire: Wire,
    point: Point,
    side: BoardSide,
}

impl EdgeAttachment {
    fn extend(self, wire: Wire, old: BoardBounds, new: BoardBounds) -> Option<Wire> {
        let scale = wire.scale.get();
        let expanded = match self.side {
            BoardSide::Top => new.min.y < old.min.y,
            BoardSide::Right => new.max.x > old.max.x,
            BoardSide::Bottom => new.max.y > old.max.y,
            BoardSide::Left => new.min.x < old.min.x,
        };
        if !expanded {
            return None;
        }

        let point = match self.side {
            BoardSide::Top => Point::new(self.point.x, new.min.y.checked_sub(scale)?),
            BoardSide::Right => Point::new(new.max.x, self.point.y),
            BoardSide::Bottom => Point::new(self.point.x, new.max.y),
            BoardSide::Left => Point::new(new.min.x.checked_sub(scale)?, self.point.y),
        };
        let (start, end) = if self.point == self.wire.start {
            (point, wire.end)
        } else {
            (wire.start, point)
        };
        Wire::new(start, end, wire.scale).ok()
    }
}

impl Default for BoardBounds {
    fn default() -> Self {
        Self {
            min: Point::new(0, 0),
            max: Point::new(1, 1),
        }
    }
}

impl BoardBounds {
    fn from_grid(grid: &LogicGrid) -> Option<Self> {
        Self::from_grid_and_previews(grid, &[], &[])
    }

    fn from_grid_and_previews(
        grid: &LogicGrid,
        preview_components: &[Component],
        preview_wires: &[Wire],
    ) -> Option<Self> {
        Self::from_geometry(grid, preview_components, preview_wires, &BTreeSet::new())
    }

    fn from_grid_ignoring_endpoints(
        grid: &LogicGrid,
        ignored_endpoints: &BTreeSet<(Point, Scale)>,
    ) -> Option<Self> {
        Self::from_geometry(grid, &[], &[], ignored_endpoints)
    }

    fn from_geometry(
        grid: &LogicGrid,
        preview_components: &[Component],
        preview_wires: &[Wire],
        ignored_endpoints: &BTreeSet<(Point, Scale)>,
    ) -> Option<Self> {
        let mut bounds = None;
        for component in grid.components().chain(preview_components) {
            Self::include_component_in(&mut bounds, component);
        }
        let wires: Vec<_> = grid
            .wires()
            .iter()
            .copied()
            .chain(preview_wires.iter().copied())
            .collect();
        for wire in &wires {
            Self::include_wire_interior_in(&mut bounds, *wire);
        }
        let mut endpoint_counts = BTreeMap::new();
        for wire in &wires {
            for point in [wire.start, wire.end] {
                *endpoint_counts.entry((point, wire.scale)).or_insert(0) += 1;
            }
        }
        for ((point, scale), count) in &endpoint_counts {
            if *count >= 2 && !ignored_endpoints.contains(&(*point, *scale)) {
                if let Some(endpoint) = Self::endpoint_bounds(*point, *scale) {
                    Self::include_rect(&mut bounds, endpoint.min, endpoint.max);
                }
            }
        }

        let core_bounds = bounds;
        for wire in wires {
            for (point, endpoint, attached) in Self::wire_endpoints(wire, core_bounds) {
                if ignored_endpoints.contains(&(point, wire.scale)) {
                    continue;
                }
                if endpoint_counts
                    .get(&(point, wire.scale))
                    .is_some_and(|count| *count >= 2)
                {
                    continue;
                }
                if !attached {
                    Self::include_rect(&mut bounds, endpoint.min, endpoint.max);
                }
            }
        }
        bounds
    }

    fn edge_attachments(grid: &LogicGrid, bounds: Self) -> Vec<EdgeAttachment> {
        let mut endpoint_counts = BTreeMap::new();
        for wire in grid.wires() {
            for point in [wire.start, wire.end] {
                *endpoint_counts.entry((point, wire.scale)).or_insert(0) += 1;
            }
        }

        let mut attachments = Vec::new();
        for wire in grid.wires() {
            for (point, _, attached) in Self::wire_endpoints(*wire, Some(bounds)) {
                if !attached
                    || endpoint_counts
                        .get(&(point, wire.scale))
                        .is_some_and(|count| *count >= 2)
                {
                    continue;
                }
                let side = if point == wire.start {
                    match wire.orientation() {
                        logicgame::grid::Orientation::Horizontal => BoardSide::Left,
                        logicgame::grid::Orientation::Vertical => BoardSide::Top,
                    }
                } else {
                    match wire.orientation() {
                        logicgame::grid::Orientation::Horizontal => BoardSide::Right,
                        logicgame::grid::Orientation::Vertical => BoardSide::Bottom,
                    }
                };
                attachments.push(EdgeAttachment {
                    wire: *wire,
                    point,
                    side,
                });
            }
        }
        attachments
    }

    fn include_component_in(bounds: &mut Option<Self>, component: &Component) {
        let Some(size) = component.size() else {
            return;
        };
        let Some(max_x) = component.position.x.checked_add(size.width) else {
            return;
        };
        let Some(max_y) = component.position.y.checked_add(size.height) else {
            return;
        };
        Self::include_rect(bounds, component.position, Point::new(max_x, max_y));
    }

    fn include_wire_interior_in(bounds: &mut Option<Self>, wire: Wire) {
        let scale = wire.scale.get();
        let Some(start) = (match wire.orientation() {
            logicgame::grid::Orientation::Horizontal => wire
                .start
                .x
                .checked_add(scale)
                .map(|x| Point::new(x, wire.start.y)),
            logicgame::grid::Orientation::Vertical => wire
                .start
                .y
                .checked_add(scale)
                .map(|y| Point::new(wire.start.x, y)),
        }) else {
            return;
        };
        let Some(end) = (match wire.orientation() {
            logicgame::grid::Orientation::Horizontal => wire
                .start
                .y
                .checked_add(scale)
                .map(|y| Point::new(wire.end.x, y)),
            logicgame::grid::Orientation::Vertical => wire
                .start
                .x
                .checked_add(scale)
                .map(|x| Point::new(x, wire.end.y)),
        }) else {
            return;
        };
        Self::include_rect(bounds, start, end);
    }

    fn wire_endpoints(wire: Wire, bounds: Option<Self>) -> Vec<(Point, Self, bool)> {
        let Some(start) = Self::endpoint_bounds(wire.start, wire.scale) else {
            return Vec::new();
        };
        let Some(end) = Self::endpoint_bounds(wire.end, wire.scale) else {
            return Vec::new();
        };
        let Some(bounds) = bounds else {
            return vec![(wire.start, start, false), (wire.end, end, false)];
        };
        let cross_axis_inside = |endpoint: Self| match wire.orientation() {
            logicgame::grid::Orientation::Horizontal => {
                bounds.min.y <= endpoint.min.y && endpoint.max.y <= bounds.max.y
            }
            logicgame::grid::Orientation::Vertical => {
                bounds.min.x <= endpoint.min.x && endpoint.max.x <= bounds.max.x
            }
        };
        let start_attached = cross_axis_inside(start)
            && match wire.orientation() {
                logicgame::grid::Orientation::Horizontal => start.max.x == bounds.min.x,
                logicgame::grid::Orientation::Vertical => start.max.y == bounds.min.y,
            };
        let end_attached = cross_axis_inside(end)
            && match wire.orientation() {
                logicgame::grid::Orientation::Horizontal => end.min.x == bounds.max.x,
                logicgame::grid::Orientation::Vertical => end.min.y == bounds.max.y,
            };
        vec![
            (wire.start, start, start_attached),
            (wire.end, end, end_attached),
        ]
    }

    fn endpoint_bounds(point: Point, scale: Scale) -> Option<Self> {
        Some(Self {
            min: point,
            max: Point::new(
                point.x.checked_add(scale.get())?,
                point.y.checked_add(scale.get())?,
            ),
        })
    }

    fn include_rect(bounds: &mut Option<Self>, min: Point, max: Point) {
        if min.x >= max.x || min.y >= max.y {
            return;
        }
        *bounds = Some(match *bounds {
            Some(bounds) => Self {
                min: Point::new(bounds.min.x.min(min.x), bounds.min.y.min(min.y)),
                max: Point::new(bounds.max.x.max(max.x), bounds.max.y.max(max.y)),
            },
            None => Self { min, max },
        });
    }

    fn as_f32(self) -> [f32; 4] {
        [
            self.min.x as f32,
            self.min.y as f32,
            self.max.x as f32,
            self.max.y as f32,
        ]
    }
}

fn world_to_screen(world: [f32; 2], camera: Camera, rect: egui::Rect) -> egui::Pos2 {
    rect.center()
        + egui::vec2(
            (world[0] - camera.center[0]) * camera.zoom,
            (world[1] - camera.center[1]) * camera.zoom,
        )
}

#[derive(Clone, Copy)]
struct WorldRect {
    min: [f32; 2],
    max: [f32; 2],
}

impl WorldRect {
    fn from_points(first: [f32; 2], second: [f32; 2]) -> Self {
        Self {
            min: [first[0].min(second[0]), first[1].min(second[1])],
            max: [first[0].max(second[0]), first[1].max(second[1])],
        }
    }

    fn intersects(self, min: [f32; 2], max: [f32; 2]) -> bool {
        self.min[0] <= max[0]
            && min[0] <= self.max[0]
            && self.min[1] <= max[1]
            && min[1] <= self.max[1]
    }
}

fn component_contains(component: &Component, point: [f32; 2]) -> bool {
    let Some(size) = component.size() else {
        return false;
    };
    let (Some(right), Some(bottom)) = (
        component.position.x.checked_add(size.width),
        component.position.y.checked_add(size.height),
    ) else {
        return false;
    };
    point[0] >= component.position.x as f32
        && point[0] <= right as f32
        && point[1] >= component.position.y as f32
        && point[1] <= bottom as f32
}

fn component_intersects(component: &Component, rect: WorldRect) -> bool {
    let Some(size) = component.size() else {
        return false;
    };
    let (Some(right), Some(bottom)) = (
        component.position.x.checked_add(size.width),
        component.position.y.checked_add(size.height),
    ) else {
        return false;
    };
    rect.intersects(
        [component.position.x as f32, component.position.y as f32],
        [right as f32, bottom as f32],
    )
}

fn point_cell_intersects(point: Point, scale: Scale, rect: WorldRect) -> bool {
    let scale = scale.get() as f32;
    rect.intersects(
        [point.x as f32, point.y as f32],
        [point.x as f32 + scale, point.y as f32 + scale],
    )
}

fn projected_wire(start: Point, end: Point, scale: Scale) -> Option<Wire> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let (start, end) = if dx.abs() >= dy.abs() {
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        (Point::new(min_x, start.y), Point::new(max_x, start.y))
    } else {
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);
        (Point::new(start.x, min_y), Point::new(start.x, max_y))
    };
    Wire::new(start, end, scale).ok()
}

fn drag_rotation(start: [f32; 2], pointer: [f32; 2]) -> Option<Rotation> {
    let dx = pointer[0] - start[0];
    let dy = pointer[1] - start[1];
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            Rotation::Right
        } else {
            Rotation::Left
        }
    } else if dy >= 0.0 {
        Rotation::Down
    } else {
        Rotation::Up
    })
}

fn oriented_component_position(anchor: Point, rotation: Rotation, scale: Scale) -> Point {
    let scale = scale.get();
    match rotation {
        Rotation::Up => Point::new(anchor.x, anchor.y - scale),
        Rotation::Right | Rotation::Down => anchor,
        Rotation::Left => Point::new(anchor.x - scale, anchor.y),
    }
}

fn nearest_wire(wires: &[Wire], point: [f32; 2], radius: f32) -> Option<Wire> {
    wires
        .iter()
        .copied()
        .filter_map(|wire| {
            let scale = wire.scale.get() as f32;
            let (min_x, max_x, min_y, max_y) = match wire.orientation() {
                logicgame::grid::Orientation::Horizontal => (
                    wire.start.x as f32,
                    wire.end.x as f32 + scale,
                    wire.start.y as f32,
                    wire.start.y as f32 + scale,
                ),
                logicgame::grid::Orientation::Vertical => (
                    wire.start.x as f32,
                    wire.start.x as f32 + scale,
                    wire.start.y as f32,
                    wire.end.y as f32 + scale,
                ),
            };
            let closest_x = point[0].clamp(min_x, max_x);
            let closest_y = point[1].clamp(min_y, max_y);
            let distance = ((point[0] - closest_x).powi(2) + (point[1] - closest_y).powi(2)).sqrt();
            (distance <= radius).then_some((distance, wire))
        })
        .min_by(|(first_distance, first), (second_distance, second)| {
            first_distance
                .total_cmp(second_distance)
                .then_with(|| first.cmp(second))
        })
        .map(|(_, wire)| wire)
}

fn nearest_wire_endpoint(wires: &[Wire], point: [f32; 2], radius: f32) -> Option<WireEndpoint> {
    wires
        .iter()
        .copied()
        .flat_map(|wire| [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end }))
        .filter_map(|endpoint| {
            let scale = endpoint.wire.scale.get() as f32;
            let endpoint_point = endpoint.point();
            let center = [
                endpoint_point.x as f32 + scale * 0.5,
                endpoint_point.y as f32 + scale * 0.5,
            ];
            let distance = ((point[0] - center[0]).powi(2) + (point[1] - center[1]).powi(2)).sqrt();
            (distance <= radius + scale * 0.5).then_some((distance, endpoint))
        })
        .min_by(|(first_distance, first), (second_distance, second)| {
            first_distance
                .total_cmp(second_distance)
                .then_with(|| first.cmp(second))
        })
        .map(|(_, endpoint)| endpoint)
}

fn graph_layout(graph: &CircuitGraph) -> (Vec<egui::Pos2>, egui::Vec2) {
    let mut columns = [Vec::new(), Vec::new(), Vec::new()];
    for (index, node) in graph.nodes.iter().enumerate() {
        let column = match node {
            GraphNode::Component { .. } => 0,
            GraphNode::Connection { .. } => 1,
            GraphNode::WireNet { .. } => 2,
        };
        columns[column].push(index);
    }

    let rows = columns.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let row_stride = GRAPH_NODE_SIZE.y + GRAPH_ROW_GAP;
    let content_height =
        rows as f32 * GRAPH_NODE_SIZE.y + rows.saturating_sub(1) as f32 * GRAPH_ROW_GAP;
    let size = egui::vec2(
        GRAPH_MARGIN * 2.0 + GRAPH_NODE_SIZE.x * 3.0 + GRAPH_COLUMN_GAP * 2.0,
        GRAPH_MARGIN * 2.0 + content_height,
    );
    let mut positions = vec![egui::Pos2::ZERO; graph.nodes.len()];

    for (column, nodes) in columns.iter().enumerate() {
        let column_height = nodes.len() as f32 * GRAPH_NODE_SIZE.y
            + nodes.len().saturating_sub(1) as f32 * GRAPH_ROW_GAP;
        let top = GRAPH_MARGIN + (content_height - column_height) / 2.0;
        let x = GRAPH_MARGIN
            + GRAPH_NODE_SIZE.x / 2.0
            + column as f32 * (GRAPH_NODE_SIZE.x + GRAPH_COLUMN_GAP);
        for (row, node) in nodes.iter().enumerate() {
            positions[*node] =
                egui::pos2(x, top + GRAPH_NODE_SIZE.y / 2.0 + row as f32 * row_stride);
        }
    }

    (positions, size)
}

fn graph_node_display(node: &GraphNode) -> (egui::Color32, &'static str, String) {
    match node {
        GraphNode::Component { component } => (
            egui::Color32::from_rgb(50, 105, 175),
            "Component",
            format!("component #{}", component.0),
        ),
        GraphNode::WireNet { wires } => (
            egui::Color32::from_rgb(45, 135, 85),
            "Wire net",
            format!("{} segment(s)", wires.len()),
        ),
        GraphNode::Connection {
            component,
            slot,
            direction,
            side,
            start,
            end,
        } => (
            egui::Color32::from_rgb(155, 95, 45),
            "Connection",
            format!(
                "#{} slot {} {direction:?} {side:?} [{start}, {end})",
                component.0, slot.0
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logicgame::grid::{ComponentSide, ConnectionDirection, ConnectionSlotId};

    fn scale(value: u8) -> Scale {
        Scale::new(value).unwrap()
    }

    fn wire(start: (i64, i64), end: (i64, i64), scale: u8) -> Wire {
        Wire::new(
            Point::new(start.0, start.1),
            Point::new(end.0, end.1),
            Scale::new(scale).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn graph_hover_maps_each_node_to_its_grid_geometry() {
        let component = ComponentId(3);
        let wire = wire((0, 0), (8, 0), 1);
        let connection = ConnectionSlot {
            id: ConnectionSlotId(2),
            direction: ConnectionDirection::Output,
            side: ComponentSide::Bottom,
            start: 4,
            end: 8,
        };
        let mut hover = GraphHover::default();

        hover.include_node(&GraphNode::Component { component });
        hover.include_node(&GraphNode::WireNet { wires: vec![wire] });
        hover.include_node(&GraphNode::Connection {
            component,
            slot: connection.id,
            direction: connection.direction,
            side: connection.side,
            start: connection.start,
            end: connection.end,
        });

        assert_eq!(hover.components, BTreeSet::from([component]));
        assert_eq!(hover.wires, BTreeSet::from([wire]));
        assert_eq!(hover.connections, BTreeSet::from([(component, connection)]));
    }

    #[test]
    fn screen_world_round_trip_and_cursor_zoom_are_stable() {
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 40.0), egui::vec2(800.0, 600.0));
        let mut camera = Camera {
            center: [12.0, -8.0],
            zoom: 20.0,
        };
        let cursor = egui::pos2(187.0, 249.0);
        let before = camera.screen_to_world(cursor, rect);
        camera.zoom_around(cursor, rect, 2.0);
        let after = camera.screen_to_world(cursor, rect);
        assert!((before[0] - after[0]).abs() < 0.0001);
        assert!((before[1] - after[1]).abs() < 0.0001);
    }

    #[test]
    fn snapping_selects_the_containing_grid_cell() {
        assert_eq!(snap_point([0.0, 0.0], scale(4)), Point::new(0, 0));
        assert_eq!(snap_point([3.99, 3.99], scale(4)), Point::new(0, 0));
        assert_eq!(snap_point([4.0, 4.0], scale(4)), Point::new(4, 4));
        assert_eq!(snap_point([-0.01, -0.01], scale(4)), Point::new(-4, -4));
        assert_eq!(snap_point([-4.0, -4.0], scale(4)), Point::new(-4, -4));
    }

    #[test]
    fn empty_grid_uses_the_one_cell_origin_board() {
        assert_eq!(
            BoardBounds::from_grid(&LogicGrid::new()).unwrap_or_default(),
            BoardBounds {
                min: Point::new(0, 0),
                max: Point::new(1, 1),
            }
        );
    }

    #[test]
    fn board_bounds_include_rotated_component_extents() {
        let mut grid = LogicGrid::new();
        grid.add_component(
            Point::new(10, -4),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );

        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(10, -4),
                max: Point::new(14, -2),
            })
        );
    }

    #[test]
    fn wire_board_bounds_exclude_only_edge_facing_endpoints() {
        let mut horizontal = None;
        BoardBounds::include_wire_interior_in(&mut horizontal, wire((-4, 6), (8, 6), 2));
        assert_eq!(
            horizontal,
            Some(BoardBounds {
                min: Point::new(-2, 6),
                max: Point::new(8, 8),
            })
        );

        let mut vertical = None;
        BoardBounds::include_wire_interior_in(&mut vertical, wire((3, -8), (3, 4), 2));
        assert_eq!(
            vertical,
            Some(BoardBounds {
                min: Point::new(3, -6),
                max: Point::new(5, 4),
            })
        );

        let mut grid = LogicGrid::new();
        grid.add_wire(wire((0, 0), (2, 0), 2));
        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(0, 0),
                max: Point::new(4, 2),
            })
        );
    }

    #[test]
    fn perpendicular_wire_can_extend_a_side_and_keep_an_external_endpoint() {
        let mut grid = LogicGrid::new();
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Not { scale: scale(2) },
        );
        grid.add_wire(wire((4, -2), (4, 4), 1));

        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(0, -1),
                max: Point::new(5, 4),
            })
        );
    }

    #[test]
    fn parallel_wire_on_a_side_expands_the_board_instead_of_attaching() {
        let mut grid = LogicGrid::new();
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Not { scale: scale(2) },
        );
        grid.add_wire(wire((0, -1), (1, -1), 1));

        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(0, -1),
                max: Point::new(2, 4),
            })
        );
    }

    #[test]
    fn shared_t_junction_is_the_component_body_and_leaves_are_attachments() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((0, 0), (0, 1), 1));
        grid.add_wire(wire((0, 1), (0, 2), 1));
        grid.add_wire(wire((0, 1), (1, 1), 1));

        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(0, 1),
                max: Point::new(1, 2),
            })
        );
    }

    #[test]
    fn expanding_an_edge_stretches_its_existing_attachment_wire() {
        let mut editor = LogicEditor::default();
        editor
            .grid
            .add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        editor.grid.add_wire(wire((0, 0), (1, 0), 1));

        editor.edit_grid_preserving_edge_attachments(|grid| {
            grid.add_component(Point::new(4, 0), Rotation::Up, ComponentKind::Led);
        });

        assert_eq!(editor.grid.wires(), &[wire((0, 0), (5, 0), 1)]);
        assert_eq!(
            BoardBounds::from_grid(&editor.grid),
            Some(BoardBounds {
                min: Point::new(0, 0),
                max: Point::new(5, 2),
            })
        );
    }

    #[test]
    fn attachment_extension_uses_the_new_edge_on_all_four_sides() {
        let old = BoardBounds {
            min: Point::new(0, 0),
            max: Point::new(4, 4),
        };
        let new = BoardBounds {
            min: Point::new(-2, -2),
            max: Point::new(7, 7),
        };
        for (attachment, expected) in [
            (
                EdgeAttachment {
                    wire: wire((-1, 1), (0, 1), 1),
                    point: Point::new(-1, 1),
                    side: BoardSide::Left,
                },
                wire((-3, 1), (0, 1), 1),
            ),
            (
                EdgeAttachment {
                    wire: wire((3, 1), (4, 1), 1),
                    point: Point::new(4, 1),
                    side: BoardSide::Right,
                },
                wire((3, 1), (7, 1), 1),
            ),
            (
                EdgeAttachment {
                    wire: wire((1, -1), (1, 0), 1),
                    point: Point::new(1, -1),
                    side: BoardSide::Top,
                },
                wire((1, -3), (1, 0), 1),
            ),
            (
                EdgeAttachment {
                    wire: wire((1, 3), (1, 4), 1),
                    point: Point::new(1, 4),
                    side: BoardSide::Bottom,
                },
                wire((1, 3), (1, 7), 1),
            ),
        ] {
            assert_eq!(attachment.extend(attachment.wire, old, new), Some(expected));
        }
    }

    #[test]
    fn board_bounds_union_components_and_wire_interiors() {
        let mut grid = LogicGrid::new();
        grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        grid.add_wire(wire((-3, 1), (5, 1), 1));

        assert_eq!(
            BoardBounds::from_grid(&grid),
            Some(BoardBounds {
                min: Point::new(-2, 0),
                max: Point::new(5, 2),
            })
        );
    }

    #[test]
    fn placement_preview_expands_the_rendered_board() {
        let mut editor = LogicEditor::default();
        editor.tool = Tool {
            kind: ToolKind::Led,
            scale: Scale::ONE,
        };
        editor.gesture = Some(Gesture::Led {
            anchor: Point::new(8, 8),
            drag_start: [8.0, 8.0],
        });

        let frame = editor.render_frame(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
            Some([9.0, 8.0]),
            None,
            &GraphHover::default(),
        );

        assert_eq!(frame.board_bounds, [8.0, 8.0, 10.0, 9.0]);
    }

    #[test]
    fn storage_bits_are_displayed_from_most_to_least_significant() {
        assert_eq!(storage_bit_indices(scale(1)), vec![0]);
        assert_eq!(storage_bit_indices(scale(4)), vec![3, 2, 1, 0]);
        assert_eq!(
            storage_bit_indices(scale(64)),
            (0_u32..64).rev().collect::<Vec<_>>()
        );
    }

    #[test]
    fn simulation_restarts_when_the_grid_changes() {
        let mut editor = LogicEditor::default();
        editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(1),
                value: 1,
            },
        );

        editor.run_simulation_step();
        assert_eq!(editor.simulation.steps, 1);
        assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1]);

        editor.grid.add_component(
            Point::new(4, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(1),
                value: 0,
            },
        );
        editor.run_simulation_step();

        assert_eq!(editor.simulation.steps, 1);
        assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1, 0]);
    }

    #[test]
    fn restarting_simulation_restores_grid_storage_values() {
        let mut editor = LogicEditor::default();
        editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(1),
                value: 1,
            },
        );
        editor.run_simulation_step();
        editor.simulation.vm.as_mut().unwrap().storage[0] = 0;

        editor.restart_simulation();

        assert_eq!(editor.simulation.steps, 0);
        assert_eq!(editor.simulation.vm.as_ref().unwrap().storage, vec![1]);
    }

    #[test]
    fn simulation_tracks_the_next_instruction() {
        let mut editor = LogicEditor::default();
        editor.simulation.vm = Some(Vm {
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
        });

        assert!(editor.begin_simulation_tick());
        assert_eq!(editor.simulation.next_instruction, 0);
        editor.execute_next_simulation_instruction();
        assert_eq!(editor.simulation.next_instruction, 1);
        assert_eq!(editor.simulation.steps, 0);
        assert!(editor.simulation.tick_in_progress);
        assert_eq!(editor.simulation.vm.as_ref().unwrap().memory, vec![7, 0]);

        editor.execute_next_simulation_instruction();
        assert_eq!(editor.simulation.next_instruction, 2);
        assert_eq!(editor.simulation.steps, 1);
        assert!(!editor.simulation.tick_in_progress);
        assert_eq!(editor.simulation.vm.as_ref().unwrap().memory, vec![7, !7]);
    }

    #[test]
    fn instructions_have_compact_display_names() {
        assert_eq!(
            format_instruction(&Instruction::Not {
                input: 2,
                output: 5,
            }),
            "NOT m2 -> m5"
        );
        assert_eq!(
            format_instruction(&Instruction::ReadStorage {
                storage: 3,
                output: 4,
            }),
            "READ s3 -> m4"
        );
        assert_eq!(
            format_instruction(&Instruction::SaveStorage {
                storage: 3,
                input: 4,
            }),
            "SAVE m4 -> s3"
        );
    }

    #[test]
    fn wire_projection_uses_the_dominant_axis() {
        assert_eq!(
            projected_wire(Point::new(1, 1), Point::new(2, 1), scale(1)),
            Some(wire((1, 1), (2, 1), 1))
        );
        assert_eq!(
            projected_wire(Point::new(0, 0), Point::new(8, 3), scale(1)),
            Some(wire((0, 0), (8, 0), 1))
        );
        assert_eq!(
            projected_wire(Point::new(8, 3), Point::new(0, 0), scale(1)),
            Some(wire((0, 3), (8, 3), 1))
        );
        assert_eq!(
            projected_wire(Point::new(0, 0), Point::new(2, -9), scale(1)),
            Some(wire((0, -9), (0, 0), 1))
        );
        assert_eq!(
            projected_wire(Point::new(2, -9), Point::new(0, 0), scale(1)),
            Some(wire((2, -9), (2, 0), 1))
        );
    }

    #[test]
    fn gate_drag_maps_to_rotation_and_input_anchor() {
        let anchor = Point::new(8, 8);
        assert_eq!(
            drag_rotation([9.5, 9.5], [13.0, 9.0]),
            Some(Rotation::Right)
        );
        assert_eq!(drag_rotation([9.5, 9.5], [8.5, 9.5]), Some(Rotation::Left));
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 8.5]), Some(Rotation::Up));
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 10.5]), Some(Rotation::Down));
        assert_eq!(
            drag_rotation([9.5, 9.5], [9.500_001, 9.5]),
            Some(Rotation::Right)
        );
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 9.5]), None);
        assert_eq!(
            oriented_component_position(anchor, Rotation::Right, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Down, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Up, scale(2)),
            Point::new(8, 6)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Left, scale(2)),
            Point::new(6, 8)
        );
    }

    #[test]
    fn wire_endpoint_hit_testing_ignores_segment_bodies() {
        let horizontal = wire((0, 0), (10, 0), 1);
        let vertical = wire((5, -5), (5, 5), 1);
        assert_eq!(
            nearest_wire_endpoint(&[vertical, horizontal], [0.5, 0.5], 0.1),
            Some(WireEndpoint {
                wire: horizontal,
                end: WireEnd::Start
            })
        );
        assert_eq!(nearest_wire_endpoint(&[horizontal], [5.0, 0.5], 0.6), None);
    }

    #[test]
    fn selection_movement_snaps_to_its_largest_scale() {
        assert_eq!(
            snapped_delta([1.0, 1.0], [12.9, -3.1], scale(8)),
            Point::new(8, -8)
        );
        assert_eq!(
            snapped_delta([1.0, 1.0], [13.1, 5.1], scale(8)),
            Point::new(16, 8)
        );
    }

    #[test]
    fn mixed_selection_moves_and_deletes_components_and_wires() {
        let mut editor = LogicEditor::default();
        let component = editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let original_wire = wire((0, 8), (16, 8), 8);
        editor.grid.add_wire(original_wire);
        editor.selection.components.insert(component);
        editor.selection.wire_endpoints.extend([
            WireEndpoint {
                wire: original_wire,
                end: WireEnd::Start,
            },
            WireEndpoint {
                wire: original_wire,
                end: WireEnd::End,
            },
        ]);

        let gesture = editor.move_gesture([0.0, 0.0]).unwrap();
        let Gesture::MoveSelection {
            scale: snap_scale,
            components,
            wires,
            ..
        } = gesture
        else {
            panic!("expected a move gesture");
        };
        assert_eq!(snap_scale, scale(8));

        editor.apply_move(&components, &wires, Point::new(8, -8));
        assert_eq!(
            editor.grid.component(component).unwrap().position,
            Point::new(8, -8)
        );
        assert_eq!(editor.grid.wires(), &[wire((8, 0), (24, 0), 8)]);
        assert!(editor.grid.validate().is_empty());

        editor.delete_selection();
        assert!(editor.grid.component(component).is_none());
        assert!(editor.grid.wires().is_empty());
        assert!(editor.selection.is_empty());
    }

    #[test]
    fn box_selection_finds_components_and_individual_wire_endpoints() {
        let mut editor = LogicEditor::default();
        let inside = editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let outside = editor.grid.add_component(
            Point::new(20, 20),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let selected_wire = wire((0, 6), (8, 6), 2);
        editor.grid.add_wire(selected_wire);

        editor.select_in_rect([-1.0, -1.0], [2.5, 8.0]);

        assert!(editor.selection.components.contains(&inside));
        assert!(!editor.selection.components.contains(&outside));
        assert!(editor.selection.wire_endpoints.contains(&WireEndpoint {
            wire: selected_wire,
            end: WireEnd::Start,
        }));
        assert!(!editor.selection.wire_endpoints.contains(&WireEndpoint {
            wire: selected_wire,
            end: WireEnd::End,
        }));
    }

    #[test]
    fn moving_one_wire_endpoint_resizes_or_deletes_the_segment() {
        let mut editor = LogicEditor::default();
        let original = wire((0, 0), (8, 0), 1);
        editor.grid.add_wire(original);
        editor.selection.wire_endpoints.insert(WireEndpoint {
            wire: original,
            end: WireEnd::End,
        });

        let Gesture::MoveSelection { wires, .. } =
            editor.move_gesture([8.5, 0.5]).expect("move gesture")
        else {
            panic!("expected a move gesture");
        };
        editor.apply_move(&[], &wires, Point::new(4, 0));
        assert_eq!(editor.grid.wires(), &[wire((0, 0), (12, 0), 1)]);

        let Gesture::MoveSelection { wires, .. } =
            editor.move_gesture([12.5, 0.5]).expect("move gesture")
        else {
            panic!("expected a move gesture");
        };
        editor.apply_move(&[], &wires, Point::new(-6, 0));
        assert_eq!(editor.grid.wires(), &[wire((0, 0), (6, 0), 1)]);

        let Gesture::MoveSelection { wires, .. } =
            editor.move_gesture([6.5, 0.5]).expect("move gesture")
        else {
            panic!("expected a move gesture");
        };
        editor.apply_move(&[], &wires, Point::new(0, 1));
        assert!(editor.grid.wires().is_empty());
        assert!(editor.selection.is_empty());
    }
}
