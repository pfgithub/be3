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
        value_mask, CircuitGraph, Component, ComponentId, ComponentKind, ComponentSide,
        ConnectionSlot, GraphNode, GraphNodeId, InputId, LogicGrid, OutputId, Point, Rotation,
        Scale, ValidationError, Wire,
    },
};
use logicgame::{challenges::ChallengeId, grid::ComponentFileRef};

use crate::{
    component_files::ComponentFiles,
    renderer::{DrawRay, DrawStub, DrawTriangle, DrawWire, GridCallback, RenderFrame, WireValue},
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const HOTBAR_WIDTH: f32 = 92.0;
/// Label drawn on an input/output component.
const LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(232, 236, 245);
/// Name drawn in the centre of a subcomponent.
const NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(232, 236, 245);
/// Port label drawn next to a subcomponent's port.
const PORT_LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(176, 188, 208);
const HOTBAR_SLOT_SIZE: f32 = 64.0;
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
    /// A user-defined component selected from the hotbar. The actual component
    /// to place is looked up via `LogicEditor::active_custom`.
    Custom,
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
            Self::Custom => "Component",
        }
    }
}

/// A slot in the hotbar: either one of the built-in tools or a user-defined
/// component compiled from a component file.
#[derive(Clone, Debug)]
enum HotbarSlot {
    Builtin(ToolKind),
    Custom {
        name: String,
        source: ComponentFileRef,
        kind: ComponentKind,
    },
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
    Subcomponent {
        anchor: Point,
        drag_start: [f32; 2],
        kind: ComponentKind,
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
    /// The grid state the shared simulation VM was compiled from; used to
    /// detect edits.
    snapshot: Option<SimulationSnapshot>,
    error: Option<String>,
    /// Maps each input port index to its slot in the VM's input addresses.
    input_slots: Vec<Option<usize>>,
    /// Maps each output port index to its slot in the VM's output addresses.
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
    /// Built-in tools followed by any user-pinned custom components.
    hotbar: Vec<HotbarSlot>,
    /// Index into `hotbar` of the selected custom slot, when `tool.kind` is
    /// `ToolKind::Custom`. `None` whenever a built-in tool is selected.
    active_custom: Option<usize>,
    /// Label applied to freely placed input/output components (outside a
    /// challenge, where labels come from the challenge port instead).
    io_label: String,
}

fn default_hotbar() -> Vec<HotbarSlot> {
    FREE_TOOLS
        .iter()
        .copied()
        .map(HotbarSlot::Builtin)
        .collect()
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
            hotbar: default_hotbar(),
            active_custom: None,
            io_label: String::new(),
        }
    }
}

impl LogicEditor {
    pub fn set_component_files(&mut self, component_files: Option<ComponentFiles>) {
        self.component_files = component_files;
    }

    /// Rebuilds the custom tail of the hotbar from persisted entries, keeping the
    /// built-in tools at the front. Any selected custom slot is deselected.
    pub fn set_custom_hotbar(&mut self, slots: Vec<(String, ComponentFileRef, ComponentKind)>) {
        self.hotbar = default_hotbar();
        self.hotbar.extend(
            slots
                .into_iter()
                .map(|(name, source, kind)| HotbarSlot::Custom { name, source, kind }),
        );
        if self.tool.kind == ToolKind::Custom {
            self.tool.kind = ToolKind::Select;
            self.active_custom = None;
        }
    }

    /// Appends a compiled custom component to the hotbar. If a slot for the same
    /// source already exists it is updated in place instead of duplicated.
    pub fn add_custom_hotbar_slot(
        &mut self,
        name: String,
        source: ComponentFileRef,
        kind: ComponentKind,
    ) {
        if let Some(slot) = self.hotbar.iter_mut().find(|slot| {
            matches!(slot, HotbarSlot::Custom { source: existing, .. } if *existing == source)
        }) {
            *slot = HotbarSlot::Custom { name, source, kind };
        } else {
            self.hotbar.push(HotbarSlot::Custom { name, source, kind });
        }
    }

    /// Unpins a custom hotbar slot, persisting the change and fixing up the
    /// selected-custom index for the removed/shifted entries.
    fn remove_hotbar_slot(&mut self, index: usize) {
        let Some(HotbarSlot::Custom { source, .. }) = self.hotbar.get(index) else {
            return;
        };
        let source = *source;
        if let Some(files) = &self.component_files {
            if let Err(error) = files.remove_hotbar(source) {
                eprintln!("failed to unpin hotbar component: {error}");
            }
        }
        self.hotbar.remove(index);
        match self.active_custom {
            Some(active) if active == index => {
                self.active_custom = None;
                if self.tool.kind == ToolKind::Custom {
                    self.tool.kind = ToolKind::Select;
                }
            }
            Some(active) if active > index => self.active_custom = Some(active - 1),
            _ => {}
        }
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

    /// The label the active challenge assigns to the input/output port at
    /// `index`, or an empty string when there is no challenge or no such port.
    fn challenge_port_label(&self, kind: ToolKind, index: usize) -> String {
        let Some(challenge) = self.challenge.as_ref() else {
            return String::new();
        };
        let ports = match kind {
            ToolKind::Output => &challenge.data.outputs,
            _ => &challenge.data.inputs,
        };
        ports
            .get(index)
            .map(|port| port.label.to_owned())
            .unwrap_or_default()
    }

    /// Draws text labels over the grid: the label on each input/output, the
    /// centre name of every subcomponent, and each subcomponent port's label
    /// next to its port. Text is an egui overlay because the wgpu grid renderer
    /// only draws triangles.
    fn draw_component_labels(&self, painter: &egui::Painter, rect: egui::Rect) {
        let zoom = self.camera.zoom;
        for component in self.grid.components() {
            let Some(size) = component.size() else {
                continue;
            };
            let center = [
                component.position.x as f32 + size.width as f32 * 0.5,
                component.position.y as f32 + size.height as f32 * 0.5,
            ];
            match &component.kind {
                ComponentKind::Input { label, .. } | ComponentKind::Output { label, .. } => {
                    if label.is_empty() {
                        continue;
                    }
                    painter.text(
                        world_to_screen(center, self.camera, rect),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional((zoom * 0.5).clamp(7.0, 28.0)),
                        LABEL_COLOR,
                    );
                }
                ComponentKind::Subcomponent { name, ports, .. } => {
                    if !name.is_empty() {
                        painter.text(
                            world_to_screen(center, self.camera, rect),
                            egui::Align2::CENTER_CENTER,
                            name,
                            egui::FontId::proportional((zoom * 0.45).clamp(8.0, 30.0)),
                            NAME_COLOR,
                        );
                    }
                    for slot in component.connection_slots() {
                        let Some(port) = ports.get(slot.id.0 as usize) else {
                            continue;
                        };
                        if port.label.is_empty() {
                            continue;
                        }
                        let mid = (slot.start + slot.end) as f32 * 0.5;
                        let left = component.position.x as f32;
                        let top = component.position.y as f32;
                        let right = left + size.width as f32;
                        let bottom = top + size.height as f32;
                        // Sit the text just inside the edge, anchored so it grows
                        // toward the component's interior.
                        let inset = 0.15;
                        let (point, anchor) = match slot.side {
                            ComponentSide::Top => ([mid, top + inset], egui::Align2::CENTER_TOP),
                            ComponentSide::Bottom => {
                                ([mid, bottom - inset], egui::Align2::CENTER_BOTTOM)
                            }
                            ComponentSide::Left => ([left + inset, mid], egui::Align2::LEFT_CENTER),
                            ComponentSide::Right => {
                                ([right - inset, mid], egui::Align2::RIGHT_CENTER)
                            }
                        };
                        painter.text(
                            world_to_screen(point, self.camera, rect),
                            anchor,
                            &port.label,
                            egui::FontId::proportional((zoom * 0.32).clamp(6.0, 16.0)),
                            PORT_LABEL_COLOR,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();

        // Per-port placement tools shown only inside a challenge: (port index,
        // label, scale) for inputs and outputs. Collected up front so the hotbar
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

        egui::Panel::left("logic-hotbar")
            .resizable(false)
            .exact_size(HOTBAR_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_hotbar(ui, &challenge_ports);
                });
            });

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
        self.draw_component_labels(&canvas.inner.1, canvas.inner.0.rect);
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

    fn show_hotbar(
        &mut self,
        ui: &mut egui::Ui,
        challenge_ports: &Option<(Vec<(usize, String, Scale)>, Vec<(usize, String, Scale)>)>,
    ) {
        let in_challenge = challenge_ports.is_some();
        let mut action: Option<HotbarAction> = None;
        let mut remove: Option<usize> = None;

        // Built-in tools followed by user-pinned custom components.
        for (index, slot) in self.hotbar.iter().enumerate() {
            // In a challenge the generic Input/Output tools are replaced by the
            // per-port buttons rendered below.
            if in_challenge {
                if let HotbarSlot::Builtin(ToolKind::Input | ToolKind::Output) = slot {
                    continue;
                }
            }
            let (label, selected) = match slot {
                HotbarSlot::Builtin(kind) => (
                    kind.label().to_string(),
                    self.tool.kind == *kind
                        && self.tool.challenge_port.is_none()
                        && self.active_custom.is_none(),
                ),
                HotbarSlot::Custom { name, .. } => {
                    (name.clone(), self.active_custom == Some(index))
                }
            };
            let response = hotbar_button(ui, selected, &label, |painter, rect| {
                paint_slot_preview(painter, rect, slot);
            });
            if response.clicked() {
                action = Some(match slot {
                    HotbarSlot::Builtin(kind) => HotbarAction::SelectBuiltin(*kind),
                    HotbarSlot::Custom { .. } => HotbarAction::SelectCustom(index),
                });
            }
            if matches!(slot, HotbarSlot::Custom { .. }) {
                response.context_menu(|ui| {
                    if ui.button("Remove from hotbar").clicked() {
                        remove = Some(index);
                        ui.close();
                    }
                });
            }
        }

        if let Some(index) = remove {
            self.remove_hotbar_slot(index);
        }

        if let Some((inputs, outputs)) = challenge_ports {
            ui.separator();
            ui.small("Challenge ports");
            let ports = inputs
                .iter()
                .map(|port| (ToolKind::Input, port))
                .chain(outputs.iter().map(|port| (ToolKind::Output, port)));
            for (kind, (port_index, label, scale)) in ports {
                let selected =
                    self.tool.kind == kind && self.tool.challenge_port == Some(*port_index);
                let prefix = match kind {
                    ToolKind::Output => "Out",
                    _ => "In",
                };
                let text = format!("{prefix} {label}");
                let response = hotbar_button(ui, selected, &text, |painter, rect| {
                    paint_port_glyph(painter, rect, kind);
                });
                if response.clicked() {
                    action = Some(HotbarAction::SelectPort(kind, *port_index, *scale));
                }
            }
        }

        match action {
            Some(HotbarAction::SelectBuiltin(kind)) => {
                self.tool.kind = kind;
                self.tool.challenge_port = None;
                self.active_custom = None;
                self.gesture = None;
                self.configured_storage = None;
                if kind != ToolKind::Select {
                    self.selection.clear();
                }
            }
            Some(HotbarAction::SelectCustom(index)) => {
                self.tool.kind = ToolKind::Custom;
                self.tool.challenge_port = None;
                self.active_custom = Some(index);
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            Some(HotbarAction::SelectPort(kind, index, scale)) => {
                self.tool.kind = kind;
                self.tool.challenge_port = Some(index);
                self.tool.scale = scale;
                self.active_custom = None;
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            None => {}
        }

        // Scale controls. They have no effect on custom components for now.
        ui.separator();
        if matches!(self.tool.kind, ToolKind::MergerSplitter) {
            ui.small("Input scale");
            scale_buttons(ui, &mut self.tool.scale);
            ui.small("Output scale");
            scale_buttons(ui, &mut self.tool.merger_out_scale);
        } else {
            ui.small("Scale");
            scale_buttons(ui, &mut self.tool.scale);
        }

        // A label for freely placed inputs/outputs. Inside a challenge the label
        // is fixed by the chosen port, so the field is not offered.
        if matches!(self.tool.kind, ToolKind::Input | ToolKind::Output)
            && self.tool.challenge_port.is_none()
        {
            ui.separator();
            ui.small("Label");
            ui.add(egui::TextEdit::singleline(&mut self.io_label).desired_width(f32::INFINITY));
        }

        ui.separator();
        ui.small("Middle drag: pan");
        ui.small("Wheel: zoom");
        ui.small("Shift: add/remove");
        ui.small("Delete: remove");
        ui.small("Esc: cancel");
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

    /// Recompiles the shared simulation VM for challenge testing if the grid
    /// changed since it was built, resetting any results. Cheap when the grid
    /// is unchanged.
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

    /// Re-runs the test from the start through `tick` (inclusive) so the wires
    /// reflect the inputs of that row.
    fn challenge_test_seek(&mut self, tick: usize) {
        self.challenge_test_reset();
        for _ in 0..=tick {
            self.advance_challenge_test_tick();
        }
    }

    fn challenge_test_run_all(&mut self) {
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
    fn advance_challenge_test_tick(&mut self) {
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

    fn show_challenge(&mut self, context: &egui::Context) {
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
        // In challenge mode, the wires should reflect the shared simulation VM so
        // that stepping the test visibly drives the circuit.
        let simulation_vm = self
            .simulation
            .snapshot
            .as_ref()
            .filter(|simulation_snapshot| *simulation_snapshot == snapshot)
            .and_then(|_| self.simulation.vm.as_ref());
        let root_memory = simulation_vm.map(Vm::root_memory).unwrap_or_default();

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
            // The custom component to place, resolved before the match so the
            // arm does not borrow `self` while assigning `self.gesture`.
            let custom_kind = (self.tool.kind == ToolKind::Custom)
                .then(|| self.active_custom)
                .flatten()
                .and_then(|index| self.hotbar.get(index))
                .and_then(|slot| match slot {
                    HotbarSlot::Custom { kind, .. } => Some(kind.clone()),
                    HotbarSlot::Builtin(_) => None,
                });
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
                ToolKind::Custom => custom_kind.map(|kind| Gesture::Subcomponent {
                    anchor: snap_point(world, kind.snap()),
                    drag_start: world,
                    kind,
                }),
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
                                let label = self.challenge_port_label(ToolKind::Input, port);
                                self.grid.add_component_with_explicit_io(
                                    position,
                                    rotation,
                                    ComponentKind::Input {
                                        scale,
                                        id: InputId::from_u128(port as u128),
                                        label,
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
                                        label: self.io_label.clone(),
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
                                let label = self.challenge_port_label(ToolKind::Output, port);
                                self.grid.add_component_with_explicit_io(
                                    position,
                                    rotation,
                                    ComponentKind::Output {
                                        scale,
                                        id: OutputId::from_u128(port as u128),
                                        label,
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
                                        label: self.io_label.clone(),
                                    },
                                );
                            }
                        }
                    }
                }
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        let position = subcomponent_placement_position(anchor, rotation, &kind);
                        self.grid.add_component(position, rotation, kind);
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
                                label: String::new(),
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
                                label: String::new(),
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
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: subcomponent_placement_position(*anchor, rotation, kind),
                            rotation,
                            kind: kind.clone(),
                        };
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
/// Renders the challenge test table. The row whose inputs are currently driven
/// onto the wires (the last executed tick) is highlighted. Returns the tick of a
/// row the user clicked, if any, so the caller can seek the test to it.
fn challenge_test_table(
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

enum HotbarAction {
    SelectBuiltin(ToolKind),
    SelectCustom(usize),
    SelectPort(ToolKind, usize, Scale),
}

fn color32(color: [f32; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        (color[3] * 255.0).round() as u8,
    )
}

fn scale_buttons(ui: &mut egui::Ui, scale: &mut Scale) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for value in SCALES {
            let candidate = Scale::new(value).expect("hotbar scale is valid");
            if ui
                .selectable_label(*scale == candidate, format!("{value}x"))
                .clicked()
            {
                *scale = candidate;
            }
        }
    });
}

/// A square hotbar slot: a framed button with a glyph preview painted in its top
/// portion and a label across the bottom. Returns the click response.
fn hotbar_button(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    paint_preview: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
        egui::Sense::click(),
    );
    let visuals = ui.visuals();
    let (bg, stroke) = if selected {
        (
            visuals.selection.bg_fill,
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if response.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            egui::Stroke::new(1.0, visuals.widgets.hovered.bg_stroke.color),
        )
    } else {
        (
            visuals.widgets.inactive.bg_fill,
            egui::Stroke::new(1.0, visuals.widgets.inactive.bg_stroke.color),
        )
    };
    let text_color = visuals.text_color();
    let painter = ui.painter();
    painter.rect(rect, 4.0, bg, stroke, egui::StrokeKind::Inside);
    let inner = rect.shrink(4.0);
    let preview_rect =
        egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), inner.height() * 0.66));
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(inner.min.x, preview_rect.max.y),
        egui::pos2(inner.max.x, inner.max.y),
    );
    paint_preview(painter, preview_rect);
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.0),
        text_color,
    );
    response.on_hover_text(label)
}

fn glyph_point(rect: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(
        rect.min.x + x * rect.width(),
        rect.min.y + y * rect.height(),
    )
}

fn glyph_inset(rect: egui::Rect, x0: f32, y0: f32, x1: f32, y1: f32) -> egui::Rect {
    egui::Rect::from_min_max(glyph_point(rect, x0, y0), glyph_point(rect, x1, y1))
}

fn paint_glyph_box(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_stroke(
        glyph_inset(rect, 0.08, 0.08, 0.92, 0.92),
        0.0,
        egui::Stroke::new(1.5, color),
        egui::StrokeKind::Inside,
    );
}

fn paint_glyph_triangle(
    painter: &egui::Painter,
    rect: egui::Rect,
    points: [[f32; 2]; 3],
    color: egui::Color32,
) {
    painter.add(egui::Shape::convex_polygon(
        points
            .iter()
            .map(|point| glyph_point(rect, point[0], point[1]))
            .collect(),
        color,
        egui::Stroke::NONE,
    ));
}

fn paint_glyph_diamond(
    painter: &egui::Painter,
    rect: egui::Rect,
    cx: f32,
    cy: f32,
    radius: f32,
    color: egui::Color32,
) {
    let center = glyph_point(rect, cx, cy);
    let rx = radius * rect.width();
    let ry = radius * rect.height();
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(center.x, center.y - ry),
            egui::pos2(center.x + rx, center.y),
            egui::pos2(center.x, center.y + ry),
            egui::pos2(center.x - rx, center.y),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

fn paint_port_glyph(painter: &egui::Painter, rect: egui::Rect, kind: ToolKind) {
    let accent = match kind {
        ToolKind::Output => color32(DrawTriangle::OUTPUT_COLOR),
        _ => color32(DrawTriangle::INPUT_COLOR),
    };
    paint_glyph_box(painter, rect, color32(DrawTriangle::GATE_COLOR));
    paint_glyph_triangle(
        painter,
        rect,
        [[0.28, 0.52], [0.72, 0.52], [0.5, 0.2]],
        accent,
    );
    painter.rect_filled(glyph_inset(rect, 0.42, 0.52, 0.58, 0.82), 0.0, accent);
}

fn paint_slot_preview(painter: &egui::Painter, rect: egui::Rect, slot: &HotbarSlot) {
    let gate = color32(DrawTriangle::GATE_COLOR);
    match slot {
        HotbarSlot::Builtin(ToolKind::Select) => {
            paint_glyph_triangle(painter, rect, [[0.3, 0.15], [0.3, 0.78], [0.46, 0.6]], gate);
            paint_glyph_triangle(
                painter,
                rect,
                [[0.3, 0.15], [0.46, 0.6], [0.62, 0.55]],
                gate,
            );
        }
        HotbarSlot::Builtin(ToolKind::Wire) => {
            painter.line_segment(
                [glyph_point(rect, 0.15, 0.85), glyph_point(rect, 0.85, 0.15)],
                egui::Stroke::new(3.0, color32(DrawTriangle::WIRE_COLOR)),
            );
        }
        HotbarSlot::Builtin(ToolKind::Not) => {
            paint_glyph_box(painter, rect, gate);
            paint_glyph_triangle(
                painter,
                rect,
                [[0.12, 0.82], [0.88, 0.82], [0.5, 0.12]],
                gate,
            );
        }
        HotbarSlot::Builtin(ToolKind::MergerSplitter) => {
            paint_glyph_box(painter, rect, gate);
            for triangle in [
                [[0.12, 0.82], [0.88, 0.82], [0.62, 0.5]],
                [[0.12, 0.18], [0.62, 0.5], [0.88, 0.18]],
            ] {
                paint_glyph_triangle(painter, rect, triangle, gate);
            }
        }
        HotbarSlot::Builtin(ToolKind::Led) => {
            paint_glyph_box(painter, rect, gate);
            paint_glyph_diamond(painter, rect, 0.5, 0.5, 0.3, gate);
        }
        HotbarSlot::Builtin(ToolKind::Storage) => {
            paint_glyph_box(painter, rect, gate);
            painter.rect_stroke(
                glyph_inset(rect, 0.22, 0.18, 0.78, 0.82),
                0.0,
                egui::Stroke::new(1.5, gate),
                egui::StrokeKind::Inside,
            );
        }
        HotbarSlot::Builtin(ToolKind::ConfigureStorage) => {
            paint_glyph_box(painter, rect, gate);
            painter.rect_stroke(
                glyph_inset(rect, 0.22, 0.18, 0.78, 0.82),
                0.0,
                egui::Stroke::new(1.5, gate),
                egui::StrokeKind::Inside,
            );
            paint_glyph_diamond(
                painter,
                rect,
                0.5,
                0.5,
                0.12,
                color32(DrawTriangle::HIGHLIGHT_COLOR),
            );
        }
        HotbarSlot::Builtin(ToolKind::Input) => paint_port_glyph(painter, rect, ToolKind::Input),
        HotbarSlot::Builtin(ToolKind::Output) => paint_port_glyph(painter, rect, ToolKind::Output),
        HotbarSlot::Builtin(ToolKind::Custom) => paint_glyph_box(painter, rect, gate),
        HotbarSlot::Custom { .. } => {
            paint_glyph_box(painter, rect, gate);
            for (x, y) in [(0.5, 0.08), (0.5, 0.92)] {
                painter.circle_filled(
                    glyph_point(rect, x, y),
                    2.5,
                    color32(DrawTriangle::WIRE_COLOR),
                );
            }
        }
    }
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

/// Places a custom component so the snapped click anchor sits on its leading
/// edge for the dragged facing, mirroring how the built-in tools anchor.
fn subcomponent_placement_position(
    anchor: Point,
    rotation: Rotation,
    kind: &ComponentKind,
) -> Point {
    let probe = Component {
        id: ComponentId(u64::MAX),
        position: anchor,
        rotation,
        kind: kind.clone(),
    };
    let Some(size) = probe.size() else {
        return anchor;
    };
    match rotation {
        Rotation::Up => Point::new(anchor.x, anchor.y - size.height),
        Rotation::Right | Rotation::Down => anchor,
        Rotation::Left => Point::new(anchor.x - size.width, anchor.y),
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
