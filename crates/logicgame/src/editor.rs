use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use eframe::egui::{self, PointerButton};
use logicgame::{
    challenges::generate_challenge,
    execution::{Component as ExecutionComponent, Instruction, Pc, Vm},
};
use logicgame::{
    challenges::Challenge,
    grid::{
        value_mask, CircuitGraph, Component, ComponentId, ComponentKind, ConnectionSlot, GraphNode,
        GraphNodeId, InputId, LogicGrid, OutputId, Point, Rotation, Scale, ValidationError, Wire,
    },
};
use logicgame::{challenges::ChallengeId, grid::ComponentFileRef};

use crate::{
    component_files::{ComponentFileDrag, ComponentFiles},
    renderer::{DrawRay, DrawStub, DrawTriangle, DrawWire, GridCallback, RenderFrame, WireValue},
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const GRAPH_NODE_SIZE: egui::Vec2 = egui::vec2(150.0, 48.0);
const GRAPH_COLUMN_GAP: f32 = 70.0;
const GRAPH_ROW_GAP: f32 = 18.0;
const GRAPH_MARGIN: f32 = 24.0;
const FREE_TOOLS: [ToolKind; 9] = [
    ToolKind::Select,
    ToolKind::Wire,
    ToolKind::Not,
    ToolKind::MergerSplitter,
    ToolKind::Led,
    ToolKind::Storage,
    ToolKind::Input,
    ToolKind::Output,
    ToolKind::ConfigureStorage,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Select,
    Wire,
    Not,
    MergerSplitter,
    Led,
    Storage,
    Input,
    Output,
    ConfigureStorage,
}

impl ToolKind {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Not => "NOT gate",
            Self::MergerSplitter => "Merger/Splitter",
            Self::Led => "LED",
            Self::Storage => "Storage",
            Self::Input => "Input",
            Self::Output => "Output",
            Self::ConfigureStorage => "Configure storage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tool {
    kind: ToolKind,
    scale: Scale,
    merger_out_scale: Scale,
    // When placing an Input/Output inside a challenge, the port index it binds
    // to (its identity becomes `InputId`/`OutputId::from_u128(port)`). `None`
    // for ordinary placement, which generates a fresh id.
    challenge_port: Option<usize>,
}

impl Tool {
    fn conversion_scales(self) -> (Scale, Scale) {
        match self.kind {
            ToolKind::MergerSplitter => (self.scale, self.merger_out_scale),
            _ => (self.scale, self.scale),
        }
    }

    fn snap(self) -> Scale {
        match self.kind {
            ToolKind::MergerSplitter => self.scale.max(self.merger_out_scale),
            _ => self.scale,
        }
    }
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
    MergerSplitter {
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
    Input {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Output {
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
    error: Option<String>,
    input_values: Vec<u64>,
    steps: u64,
    instruction_selection: SimulationInstructionSelection,
    tick_in_progress: bool,
}

#[derive(Debug)]
struct ChallengeState {
    id: ChallengeId,
    data: Challenge,
    test: ChallengeTest,
    /// One-shot flag, set the frame the test transitions to a full pass and
    /// cleared by `take_challenge_passed`.
    passed_event: bool,
}

/// Runs the open challenge solution against the challenge's expected values.
#[derive(Debug, Default)]
struct ChallengeTest {
    /// The grid state the `vm` was compiled from; used to detect edits.
    snapshot: Option<SimulationSnapshot>,
    vm: Option<Vm>,
    error: Option<String>,
    /// Maps each input port index to its slot in `vm.input_addresses()`.
    input_slots: Vec<Option<usize>>,
    /// Maps each output port index to its slot in `vm.output_addresses()`.
    output_slots: Vec<Option<usize>>,
    /// Number of ticks executed so far.
    next_tick: usize,
    /// Actual output values, indexed `[output_port][tick]`; each inner vec has
    /// length `next_tick`.
    actual: Vec<Vec<u64>>,
    /// Whether any executed tick produced a wrong output.
    mismatched: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum SimulationInstructionSelection {
    #[default]
    Active,
    ReturnFrame(usize),
    Component(Rc<ExecutionComponent>),
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
                scale,
            } => {
                self.connections.insert((
                    *component,
                    ConnectionSlot {
                        id: *slot,
                        direction: *direction,
                        side: *side,
                        start: *start,
                        end: *end,
                        scale: *scale,
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
    component_files: Option<ComponentFiles>,
    challenge: Option<ChallengeState>,
}

pub struct ComponentFileDrop {
    pub file: ComponentFileRef,
    pub position: Point,
}

impl Default for LogicEditor {
    fn default() -> Self {
        Self {
            grid: LogicGrid::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
                merger_out_scale: Scale::new(4).expect("default scale is valid"),
                challenge_port: None,
            },
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
            configured_storage: None,
            simulation: Simulation::default(),
            component_files: None,
            challenge: None,
        }
    }
}

impl LogicEditor {
    pub fn set_component_files(&mut self, component_files: Option<ComponentFiles>) {
        self.component_files = component_files;
    }

    pub fn grid(&self) -> &LogicGrid {
        &self.grid
    }

    pub fn replace_grid(&mut self, grid: LogicGrid) {
        self.grid = grid;
        self.camera = Camera::default();
        self.gesture = None;
        self.selection.clear();
        self.configured_storage = None;
        self.simulation = Simulation::default();
        self.challenge = None;
    }

    pub fn open_challenge_solution(&mut self, id: ChallengeId, grid: LogicGrid) {
        self.replace_grid(grid);
        self.tool = Tool {
            kind: ToolKind::Select,
            scale: Scale::ONE,
            merger_out_scale: Scale::ONE,
            challenge_port: None,
        };
        let challenge_data = generate_challenge(id);
        self.challenge = Some(ChallengeState {
            id,
            data: challenge_data,
            test: ChallengeTest::default(),
            passed_event: false,
        });
    }

    pub fn active_challenge_id(&self) -> Option<ChallengeId> {
        self.challenge.as_ref().map(|challenge| challenge.id)
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<ComponentFileDrop> {
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

        // Per-port placement tools shown only inside a challenge: (port index,
        // label, scale) for inputs and outputs. Collected up front so the Tools
        // closure does not borrow `self.challenge` while mutating `self.tool`.
        let challenge_ports: Option<(Vec<(usize, String, Scale)>, Vec<(usize, String, Scale)>)> =
            self.challenge.as_ref().map(|challenge| {
                let ports = |list: &[logicgame::challenges::ChallengePort]| {
                    list.iter()
                        .enumerate()
                        .map(|(index, port)| (index, port.label.to_string(), port.scale))
                        .collect::<Vec<_>>()
                };
                (
                    ports(&challenge.data.inputs),
                    ports(&challenge.data.outputs),
                )
            });

        egui::Window::new("Tools")
            .default_pos([16.0, 16.0])
            .hscroll(true)
            .vscroll(true)
            .show(&context, |ui| {
                for &kind in &FREE_TOOLS {
                    // In a challenge the generic Input/Output tools are replaced
                    // by per-port buttons rendered below.
                    if challenge_ports.is_some()
                        && matches!(kind, ToolKind::Input | ToolKind::Output)
                    {
                        continue;
                    }
                    let selected = self.tool.kind == kind && self.tool.challenge_port.is_none();
                    if ui.selectable_label(selected, kind.label()).clicked() {
                        self.tool.kind = kind;
                        self.tool.challenge_port = None;
                        self.gesture = None;
                        self.configured_storage = None;
                        if kind != ToolKind::Select {
                            self.selection.clear();
                        }
                    }
                }

                if let Some((inputs, outputs)) = &challenge_ports {
                    ui.separator();
                    ui.label("Challenge ports");
                    let mut clicked_port: Option<(ToolKind, usize, Scale)> = None;
                    let ports = inputs
                        .iter()
                        .map(|port| (ToolKind::Input, port))
                        .chain(outputs.iter().map(|port| (ToolKind::Output, port)));
                    for (kind, (index, label, scale)) in ports {
                        let selected =
                            self.tool.kind == kind && self.tool.challenge_port == Some(*index);
                        let prefix = match kind {
                            ToolKind::Output => "Output",
                            _ => "Input",
                        };
                        if ui
                            .selectable_label(selected, format!("{prefix} {label}"))
                            .clicked()
                        {
                            clicked_port = Some((kind, *index, *scale));
                        }
                    }
                    if let Some((kind, index, scale)) = clicked_port {
                        self.tool.kind = kind;
                        self.tool.challenge_port = Some(index);
                        self.tool.scale = scale;
                        self.gesture = None;
                        self.configured_storage = None;
                        self.selection.clear();
                    }
                }

                ui.separator();
                if matches!(self.tool.kind, ToolKind::MergerSplitter) {
                    ui.label("Input scale");
                    egui::ComboBox::from_id_salt("logic-tool-scale")
                        .selected_text(format!("{}x", self.tool.scale.get()))
                        .show_ui(ui, |ui| {
                            for value in SCALES {
                                let scale = Scale::new(value).expect("tool scale is valid");
                                ui.selectable_value(
                                    &mut self.tool.scale,
                                    scale,
                                    format!("{value}x"),
                                );
                            }
                        });
                    ui.label("Output scale");
                    egui::ComboBox::from_id_salt("logic-tool-output-scale")
                        .selected_text(format!("{}x", self.tool.merger_out_scale.get()))
                        .show_ui(ui, |ui| {
                            for value in SCALES {
                                let scale = Scale::new(value).expect("tool scale is valid");
                                ui.selectable_value(
                                    &mut self.tool.merger_out_scale,
                                    scale,
                                    format!("{value}x"),
                                );
                            }
                        });
                } else {
                    ui.label("Scale");
                    egui::ComboBox::from_id_salt("logic-tool-scale")
                        .selected_text(format!("{}x", self.tool.scale.get()))
                        .show_ui(ui, |ui| {
                            for value in SCALES {
                                let scale = Scale::new(value).expect("tool scale is valid");
                                ui.selectable_value(
                                    &mut self.tool.scale,
                                    scale,
                                    format!("{value}x"),
                                );
                            }
                        });
                }
                ui.separator();
                ui.small("Middle drag: pan");
                ui.small("Wheel: zoom");
                ui.small("Shift: add/remove selection");
                ui.small("Delete: remove selection");
                ui.small("Esc: cancel");
            });

        self.update_simulation_preview();
        self.show_metrics(&context);
        self.show_storage_configuration(&context);
        self.show_simulation(&context);
        self.show_challenge(&context);

        let hovered_square = canvas
            .inner
            .2
            .map(|pointer| snap_point(pointer, self.tool.snap()));
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

        let payload = canvas.inner.0.dnd_release_payload::<ComponentFileDrag>()?;
        let pointer = context.pointer_interact_pos()?;
        let world = self.camera.screen_to_world(pointer, canvas.inner.0.rect);
        Some(ComponentFileDrop {
            file: payload.file.clone(),
            position: Point::new(world[0].floor() as i64, world[1].floor() as i64),
        })
    }

    pub fn insert_subcomponent(&mut self, position: Point, kind: ComponentKind) {
        let snap = kind.snap().get();
        let position = Point::new(
            position.x.div_euclid(snap) * snap,
            position.y.div_euclid(snap) * snap,
        );
        self.grid.add_component(position, Rotation::Up, kind);
    }

    fn show_metrics(&self, context: &egui::Context) {
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

    fn show_simulation(&mut self, context: &egui::Context) {
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
                if !vm.returns.is_empty() {
                    if ui
                        .selectable_label(
                            matches!(
                                self.simulation.instruction_selection,
                                SimulationInstructionSelection::Active
                            ),
                            format!("Current: {}", simulation_component_name(&vm.pc.component)),
                        )
                        .clicked()
                    {
                        self.simulation.instruction_selection =
                            SimulationInstructionSelection::Active;
                    }
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
                            ComponentKind::Input { scale, id } => Some((id, scale)),
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

    fn run_simulation_tick(&mut self) {
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

    fn compile_simulation(&mut self, snapshot: SimulationSnapshot) {
        let previous_input_values = self.simulation.input_values.clone();
        match Vm::from_graph(&self.grid, &snapshot.graph).map_err(|error| format!("{error:?}")) {
            Ok(mut vm) => {
                if let Some(files) = &self.component_files {
                    if let Err(error) = files.load_components(&mut vm) {
                        self.simulation = Simulation {
                            snapshot: Some(snapshot),
                            vm: None,
                            error: Some(error.to_string()),
                            input_values: Vec::new(),
                            steps: 0,
                            instruction_selection: SimulationInstructionSelection::Active,
                            tick_in_progress: false,
                        };
                        return;
                    }
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

    fn prepare_simulation(&mut self) -> bool {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
        }
        self.simulation.vm.is_some()
    }

    fn update_simulation_preview(&mut self) {
        let snapshot = self.simulation_snapshot();
        if self.simulation.snapshot.as_ref() != Some(&snapshot) {
            self.compile_simulation(snapshot);
            self.run_simulation_tick();
        }
    }

    /// Returns and clears the one-shot flag set when the challenge test passes.
    pub fn take_challenge_passed(&mut self) -> bool {
        match self.challenge.as_mut() {
            Some(challenge) => std::mem::take(&mut challenge.passed_event),
            None => false,
        }
    }

    /// Recompiles the challenge test VM if the grid changed since it was built,
    /// resetting any results. Cheap when the grid is unchanged.
    fn ensure_challenge_test(&mut self) {
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

    fn compile_challenge_test(
        &self,
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

        let mut vm = match Vm::from_graph(&self.grid, &snapshot.graph) {
            Ok(vm) => vm,
            Err(error) => {
                return ChallengeTest {
                    snapshot: Some(snapshot),
                    error: Some(format!("{error:?}")),
                    input_slots,
                    output_slots,
                    actual: vec![Vec::new(); output_count],
                    ..ChallengeTest::default()
                };
            }
        };
        if let Some(files) = &self.component_files {
            if let Err(error) = files.load_components(&mut vm) {
                return ChallengeTest {
                    snapshot: Some(snapshot),
                    error: Some(error.to_string()),
                    input_slots,
                    output_slots,
                    actual: vec![Vec::new(); output_count],
                    ..ChallengeTest::default()
                };
            }
        }
        ChallengeTest {
            snapshot: Some(snapshot),
            vm: Some(vm),
            error: None,
            input_slots,
            output_slots,
            next_tick: 0,
            actual: vec![Vec::new(); output_count],
            mismatched: false,
        }
    }

    fn challenge_test_reset(&mut self) {
        if let Some(challenge) = self.challenge.as_mut() {
            // Drop the stale snapshot so the next `ensure` recompiles from scratch.
            challenge.test.snapshot = None;
        }
        self.ensure_challenge_test();
    }

    fn challenge_test_step(&mut self) {
        self.ensure_challenge_test();
        self.advance_challenge_test_tick();
    }

    fn challenge_test_run_all(&mut self) {
        self.ensure_challenge_test();
        loop {
            let more = self.challenge.as_ref().is_some_and(|challenge| {
                let test = &challenge.test;
                test.error.is_none() && test.vm.is_some() && test.next_tick < challenge.data.ticks
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
    fn advance_challenge_test_tick(&mut self) {
        let Some(challenge) = self.challenge.as_mut() else {
            return;
        };
        let data = &challenge.data;
        let test = &mut challenge.test;
        let Some(vm) = test.vm.as_mut() else {
            return;
        };
        if test.error.is_some() || test.next_tick >= data.ticks {
            return;
        }
        let tick = test.next_tick;

        vm.begin_tick();
        let input_addresses = vm.input_addresses().to_vec();
        for (port, slot) in test.input_slots.iter().enumerate() {
            let Some(address) = slot.and_then(|slot| input_addresses.get(slot).copied()) else {
                continue;
            };
            if address >= vm.root_component.memory_size {
                continue;
            }
            let scale = data.inputs[port].scale;
            let value =
                data.inputs[port].values.get(tick).copied().unwrap_or(0) & value_mask(scale);
            vm.root_memory_mut()[address] |= value;
        }
        vm.execute();

        let output_addresses = vm.output_addresses().to_vec();
        for (port, slot) in test.output_slots.iter().enumerate() {
            let mask = value_mask(data.outputs[port].scale);
            let actual = slot
                .and_then(|slot| output_addresses.get(slot).copied())
                .and_then(|address| vm.root_memory().get(address).copied())
                .map(|value| value & mask)
                .unwrap_or(0);
            let expected = data.outputs[port].values.get(tick).copied().unwrap_or(0) & mask;
            if actual != expected {
                test.mismatched = true;
            }
            test.actual[port].push(actual);
        }
        test.next_tick += 1;

        let all_ports = test.input_slots.iter().all(Option::is_some)
            && test.output_slots.iter().all(Option::is_some);
        let passed = test.next_tick == data.ticks && !test.mismatched && all_ports;
        if passed {
            challenge.passed_event = true;
        }
    }

    fn show_challenge(&mut self, context: &egui::Context) {
        if self.challenge.is_none() {
            return;
        }
        self.ensure_challenge_test();

        let mut do_step = false;
        let mut do_run = false;
        let mut do_reset = false;

        egui::Window::new("Challenge")
            .default_pos([360.0, 16.0])
            .default_size([320.0, 440.0])
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

                challenge_test_table(ui, data, test);
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
    }

    fn begin_simulation_tick(&mut self) -> bool {
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

    fn execute_next_simulation_instruction(&mut self) {
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

    fn simulation_snapshot(&self) -> SimulationSnapshot {
        SimulationSnapshot {
            components: self.grid.components().cloned().collect(),
            wires: self.grid.wires().to_vec(),
            graph: self.grid.generate_graph(),
        }
    }

    fn wire_value_indices(
        &self,
        snapshot: &SimulationSnapshot,
    ) -> (
        BTreeMap<Wire, u32>,
        BTreeMap<ComponentId, u32>,
        Vec<WireValue>,
    ) {
        let root_memory = self
            .simulation
            .snapshot
            .as_ref()
            .filter(|simulation_snapshot| *simulation_snapshot == snapshot)
            .and_then(|_| self.simulation.vm.as_ref())
            .map(Vm::root_memory)
            .unwrap_or_default();

        let mut values = Vec::new();
        let mut indices = BTreeMap::new();
        let mut address = 0;
        for node in &snapshot.graph.nodes {
            let GraphNode::WireNet { wires } = node else {
                continue;
            };
            let value_index = values.len() as u32;
            values.push(WireValue::new(
                root_memory.get(address).copied().unwrap_or_default(),
            ));
            for wire in wires {
                indices.insert(*wire, value_index);
            }
            address += 1;
        }

        let mut net_value: BTreeMap<usize, u32> = BTreeMap::new();
        for (i, node) in snapshot.graph.nodes.iter().enumerate() {
            if let GraphNode::WireNet { wires } = node {
                if let Some(&vi) = wires.first().and_then(|w| indices.get(w)) {
                    net_value.insert(i, vi);
                }
            }
        }
        let mut component_indices: BTreeMap<ComponentId, u32> = BTreeMap::new();
        for (i, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::Connection { component, .. } = node else {
                continue;
            };
            for edge in &snapshot.graph.edges {
                let other = if edge.first == GraphNodeId(i) {
                    Some(edge.second.0)
                } else if edge.second == GraphNodeId(i) {
                    Some(edge.first.0)
                } else {
                    None
                };
                if let Some(vi) = other.and_then(|j| net_value.get(&j)) {
                    component_indices.insert(*component, *vi);
                    break;
                }
            }
        }

        if values.is_empty() {
            values.push(WireValue::new(0));
        }
        (indices, component_indices, values)
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
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                for bit in storage_bit_indices(scale) {
                    let state = (value >> bit) & 1;
                    if ui.button(format!("Bit {bit}: {state}")).clicked() {
                        self.grid.toggle_storage_bit(id, bit);
                    }
                }
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
        let bounds = self.grid.bounds();
        let mut hovered_entity = None;

        egui::Window::new("Grid Debugger")
            .default_pos([700.0, 260.0])
            .default_width(240.0)
            .hscroll(true)
            .vscroll(true)
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
                        ui.label("Bounds");
                        match bounds {
                            Some(bounds) => {
                                ui.monospace(format!(
                                    "({}, {}) to ({}, {})",
                                    bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y
                                ));
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
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
            .hscroll(true)
            .vscroll(true)
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

                self.paint_generated_graph(ui, &graph, &mut hover);
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
        // Challenges are wired at single-bit scale, except for port tools, which
        // carry their port's scale so larger challenge ports place correctly.
        if self.challenge.is_some() && self.tool.challenge_port.is_none() {
            self.tool.scale = Scale::ONE;
            self.tool.merger_out_scale = Scale::ONE;
        }

        if self.tool.kind == ToolKind::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.delete_selection();
        }

        if response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    self.camera
                        .zoom_around(pointer, response.rect, (scroll * 0.002).exp());
                }
            }
        }

        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };

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
        let snapped = snap_point(world, self.tool.snap());

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
                ToolKind::MergerSplitter => Some(Gesture::MergerSplitter {
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
                ToolKind::Input => Some(Gesture::Input {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Output => Some(Gesture::Output {
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
                        self.grid.add_wire(wire);
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let scale = self.tool.scale;
                        self.grid.add_component(
                            component_placement_position(anchor, rotation, scale, ToolKind::Not),
                            rotation,
                            ComponentKind::Not { scale },
                        );
                    }
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let (input_scale, output_scale) = self.tool.conversion_scales();
                        self.grid.add_component(
                            component_placement_position(
                                anchor,
                                rotation,
                                output_scale,
                                ToolKind::MergerSplitter,
                            ),
                            rotation,
                            ComponentKind::MergerSplitter {
                                input_scale,
                                output_scale,
                            },
                        );
                    }
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        self.grid.add_component(
                            component_placement_position(
                                anchor,
                                rotation,
                                Scale::ONE,
                                ToolKind::Led,
                            ),
                            rotation,
                            ComponentKind::Led,
                        );
                    }
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let scale = self.tool.scale;
                        self.grid.add_component(
                            component_placement_position(
                                anchor,
                                rotation,
                                scale,
                                ToolKind::Storage,
                            ),
                            rotation,
                            ComponentKind::Storage { scale, value: 0 },
                        );
                    }
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world).map(|r| r.flip()) {
                        let scale = self.tool.scale;
                        let position =
                            component_placement_position(anchor, rotation, scale, ToolKind::Input);
                        match self.tool.challenge_port {
                            Some(port) => {
                                self.grid.add_component_with_explicit_io(
                                    position,
                                    rotation,
                                    ComponentKind::Input {
                                        scale,
                                        id: InputId::from_u128(port as u128),
                                    },
                                );
                            }
                            None => {
                                self.grid.add_component(
                                    position,
                                    rotation,
                                    ComponentKind::Input {
                                        scale,
                                        id: InputId::from_u128(u128::MAX),
                                    },
                                );
                            }
                        }
                    }
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let scale = self.tool.scale;
                        let position =
                            component_placement_position(anchor, rotation, scale, ToolKind::Output);
                        match self.tool.challenge_port {
                            Some(port) => {
                                self.grid.add_component_with_explicit_io(
                                    position,
                                    rotation,
                                    ComponentKind::Output {
                                        scale,
                                        id: OutputId::from_u128(port as u128),
                                    },
                                );
                            }
                            None => {
                                self.grid.add_component(
                                    position,
                                    rotation,
                                    ComponentKind::Output {
                                        scale,
                                        id: OutputId::from_u128(u128::MAX),
                                    },
                                );
                            }
                        }
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

        for wire in wires {
            self.grid.remove_wire(wire.wire);
        }
        for (id, position) in moved_components {
            self.grid.set_component_position(id, position);
        }
        for wire in &moved_wires {
            self.grid.add_wire(*wire);
        }

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
        let snapshot = self.simulation_snapshot();
        let (wire_value_indices, component_value_indices, wire_values) =
            self.wire_value_indices(&snapshot);
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
                ValidationError::InfiniteLeadBlocked { component, blocker } => {
                    bad_components.insert(component);
                    bad_components.insert(blocker);
                }
            }
        }

        let mut component_triangles = Vec::new();
        let mut draw_wires = Vec::new();
        let mut draw_rays = Vec::new();
        let mut wire_triangles = Vec::new();
        for component in self.grid.components() {
            let ray_color = match &component.kind {
                ComponentKind::Input { .. } => DrawTriangle::INPUT_COLOR,
                ComponentKind::Output { .. } => DrawTriangle::OUTPUT_COLOR,
                _ => DrawTriangle::WIRE_COLOR,
            };
            let ray_value_index = component_value_indices
                .get(&component.id)
                .copied()
                .unwrap_or_default();
            draw_rays.extend(DrawRay::from_component(
                component,
                ray_color,
                ray_value_index,
            ));
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
        // Draw a wire stub on every port the circuit graph reports as wired to
        // an actual wire net, so the wire reads as entering the component no
        // matter where along the wire the contact was made. The stub carries the
        // connected net's value index so it shows the same on/off value as the
        // wire. Ports joined directly to a touching neighbour belong to an empty
        // (wireless) net and draw nothing, so abutting components have no stub.
        let mut connection_stubs = Vec::new();
        for (index, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::Connection {
                component,
                slot,
                direction,
                side,
                start,
                end,
                scale,
            } = node
            else {
                continue;
            };
            let Some(component) = self.grid.component(*component) else {
                continue;
            };
            let node_id = GraphNodeId(index);
            let net_value_index = snapshot.graph.edges.iter().find_map(|edge| {
                let other = if edge.first == node_id {
                    edge.second
                } else if edge.second == node_id {
                    edge.first
                } else {
                    return None;
                };
                let GraphNode::WireNet { wires } = &snapshot.graph.nodes[other.0] else {
                    return None;
                };
                wires
                    .first()
                    .and_then(|wire| wire_value_indices.get(wire).copied())
            });
            let Some(value_index) = net_value_index else {
                continue;
            };
            connection_stubs.push(DrawStub::for_connection(
                component,
                ConnectionSlot {
                    id: *slot,
                    direction: *direction,
                    side: *side,
                    start: *start,
                    end: *end,
                    scale: *scale,
                },
                DrawTriangle::WIRE_COLOR,
                value_index,
            ));
        }
        for wire in self.grid.wires() {
            let color = if hovered_entity == Some(DebugEntity::Wire(*wire))
                || graph_hover.wires.contains(wire)
            {
                DrawTriangle::HIGHLIGHT_COLOR
            } else if bad_wires.contains(wire) {
                DrawTriangle::ERROR_COLOR
            } else {
                DrawTriangle::WIRE_COLOR
            };
            draw_wires.push(DrawWire::new(
                *wire,
                color,
                wire_value_indices.get(wire).copied().unwrap_or_default(),
            ));
            wire_triangles.extend(DrawTriangle::wire_endpoints(*wire, color));
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
            let snapped = snap_point(pointer, self.tool.snap());
            match self.gesture.as_ref() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(*start, snapped, self.tool.scale) {
                        wire_triangles
                            .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                                ToolKind::Not,
                            ),
                            rotation,
                            kind: ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        };
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let (input_scale, output_scale) = self.tool.conversion_scales();
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                output_scale,
                                ToolKind::MergerSplitter,
                            ),
                            rotation,
                            kind: ComponentKind::MergerSplitter {
                                input_scale,
                                output_scale,
                            },
                        };
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
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                Scale::ONE,
                                ToolKind::Led,
                            ),
                            rotation,
                            kind: ComponentKind::Led,
                        };
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
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                                ToolKind::Storage,
                            ),
                            rotation,
                            kind: ComponentKind::Storage {
                                scale: self.tool.scale,
                                value: 0,
                            },
                        };
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer).map(|r| r.flip()) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                                ToolKind::Input,
                            ),
                            rotation,
                            kind: ComponentKind::Input {
                                scale: self.tool.scale,
                                id: InputId::from_u128(u128::MAX),
                            },
                        };
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: component_placement_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                                ToolKind::Output,
                            ),
                            rotation,
                            kind: ComponentKind::Output {
                                scale: self.tool.scale,
                                id: OutputId::from_u128(u128::MAX),
                            },
                        };
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
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
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                    for wire in wires {
                        if let Some(wire) = move_selected_wire(*wire, delta) {
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
        if let Some(bounds) = self.grid.bounds() {
            wire_triangles.extend(DrawTriangle::bounds(bounds, 2.0 / self.camera.zoom));
        }

        RenderFrame {
            viewport_size: [rect.width(), rect.height()],
            camera_center: self.camera.center,
            zoom: self.camera.zoom,
            grid_scale: self.tool.snap().get() as f32,
            wires: draw_wires,
            wire_values,
            rays: draw_rays,
            stubs: connection_stubs,
            triangles: wire_triangles,
        }
    }
}

fn component_kind_name(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Not { .. } => "NOT gate",
        ComponentKind::MergerSplitter {
            input_scale,
            output_scale,
        } if input_scale <= output_scale => "Merger",
        ComponentKind::MergerSplitter { .. } => "Splitter",
        ComponentKind::Led => "LED",
        ComponentKind::Storage { .. } => "Storage",
        ComponentKind::Input { .. } => "Input",
        ComponentKind::Output { .. } => "Output",
        ComponentKind::Subcomponent { .. } => "Subcomponent",
    }
}

fn storage_bit_indices(scale: Scale) -> Vec<u32> {
    (0..scale.get() as u32).rev().collect()
}

struct SimulationInstructionView<'a> {
    name: String,
    component: &'a Rc<ExecutionComponent>,
    instructions: &'a [Instruction],
    next_instruction: Option<usize>,
}

fn simulation_instruction_view<'a>(
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

fn simulation_pc_instruction_view<'a>(
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

fn simulation_component_name(component: &ExecutionComponent) -> String {
    component
        .source_hash
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "component".to_owned())
}

fn simulation_value_row(ui: &mut egui::Ui, label: String, value: u64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.monospace(format!("0x{value:016x}"));
        ui.weak(format!("({value})"));
    });
}

fn apply_input_values(vm: &mut Vm, values: &[u64]) {
    for (&address, value) in vm.input_addresses().to_vec().iter().zip(values) {
        if address < vm.root_component.memory_size {
            vm.root_memory_mut()[address] |= *value;
        }
    }
}

/// Maps each challenge port index to its slot in the VM's input/output address
/// list. Ports placed with `from_index(port)` as their id resolve to their dense
/// (sorted-id) position; a missing port maps to `None`.
fn challenge_port_slots<T: Ord + Copy>(
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
fn challenge_test_table(ui: &mut egui::Ui, data: &Challenge, test: &ChallengeTest) {
    const CELL_WIDTH: f32 = 52.0;
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let error_color = ui.visuals().error_fg_color;
    let weak_color = ui.visuals().weak_text_color();

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

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(320.0)
        .show_rows(ui, row_height, data.ticks, |ui, range| {
            for tick in range {
                ui.horizontal(|ui| {
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
                });
            }
        });
}

fn format_instruction(instruction: &Instruction) -> String {
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

fn component_placement_position(
    anchor: Point,
    rotation: Rotation,
    scale: Scale,
    kind: ToolKind,
) -> Point {
    if matches!(
        kind,
        ToolKind::MergerSplitter | ToolKind::Input | ToolKind::Output
    ) {
        return anchor;
    }

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
            scale,
        } => (
            egui::Color32::from_rgb(155, 95, 45),
            "Connection",
            format!(
                "#{} slot {} {direction:?} {side:?} [{start}, {end}) {}x",
                component.0,
                slot.0,
                scale.get()
            ),
        ),
    }
}

#[cfg(test)]
mod tests;
