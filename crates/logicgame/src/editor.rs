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
    component_files::{ComponentFiles, SaveHotbarSlot},
    renderer::{
        DrawRay, DrawStub, DrawTriangle, DrawValueTriangle, DrawWire, GridCallback, RenderFrame,
        WireValue,
    },
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const HOTBAR_COLUMN_WIDTH: f32 = 92.0;
const HOTBAR_COLUMN_GAP: f32 = 8.0;
const SIDE_PANEL_HORIZONTAL_MARGIN: f32 = 16.0;
const HOTBAR_WIDTH: f32 =
    HOTBAR_COLUMN_WIDTH * 2.0 + HOTBAR_COLUMN_GAP + SIDE_PANEL_HORIZONTAL_MARGIN;
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
const HOTBAR_KEYS: [egui::Key; 10] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
    egui::Key::Num0,
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
    /// to place is looked up via `LogicEditor::active_hotbar_slot`.
    Custom,
}

impl ToolKind {
    fn id(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Wire => "wire",
            Self::Not => "not",
            Self::MergerSplitter => "merger_splitter",
            Self::Led => "led",
            Self::Storage => "storage",
            Self::Input => "input",
            Self::Output => "output",
            Self::ConfigureStorage => "configure_storage",
            Self::Custom => "custom",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "select" => Self::Select,
            "wire" => Self::Wire,
            "not" => Self::Not,
            "merger_splitter" => Self::MergerSplitter,
            "led" => Self::Led,
            "storage" => Self::Storage,
            "input" => Self::Input,
            "output" => Self::Output,
            "configure_storage" => Self::ConfigureStorage,
            "custom" => Self::Custom,
            _ => return None,
        })
    }

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

    fn places_component(self) -> bool {
        matches!(
            self,
            Self::Not
                | Self::MergerSplitter
                | Self::Led
                | Self::Storage
                | Self::Input
                | Self::Output
                | Self::Custom
        )
    }
}

/// A slot in the hotbar: either one of the built-in tools or a user-defined
/// component compiled from a component file.
#[derive(Clone, Debug)]
enum HotbarSlot {
    Builtin(ToolKind),
    Locked {
        name: String,
    },
    Folder {
        name: String,
        slots: Vec<HotbarSlot>,
    },
    Custom {
        name: String,
        source: ComponentFileRef,
        kind: ComponentKind,
    },
}

impl HotbarSlot {
    fn label(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.label(),
            Self::Locked { name } | Self::Folder { name, .. } | Self::Custom { name, .. } => name,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tool {
    kind: ToolKind,
    scale: Scale,
    merger_out_scale: Scale,
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
    placement_rotation: Rotation,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
    configured_storage: Option<ComponentId>,
    simulation: Simulation,
    component_files: Option<ComponentFiles>,
    challenge: Option<ChallengeState>,
    /// Root hotbar slots. Folders can contain any other hotbar slot.
    hotbar: Vec<HotbarSlot>,
    /// Path into `hotbar` of the currently open folder. Empty means root.
    active_hotbar_folder: Vec<usize>,
    /// Path into `hotbar` of the selected item, when it came from the hotbar.
    active_hotbar_slot: Option<Vec<usize>>,
    /// Slot currently being dragged for hotbar reordering/nesting.
    hotbar_drag: Option<Vec<usize>>,
    confirm_hotbar_reset: bool,
    /// Label applied to freely placed input/output components (outside a
    /// challenge, where labels come from the challenge port instead).
    io_label: String,
}

impl Default for LogicEditor {
    fn default() -> Self {
        Self {
            grid: LogicGrid::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
                merger_out_scale: Scale::new(4).expect("default scale is valid"),
            },
            placement_rotation: Rotation::Right,
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
            configured_storage: None,
            simulation: Simulation::default(),
            component_files: None,
            challenge: None,
            hotbar: default_hotbar(),
            active_hotbar_folder: Vec::new(),
            active_hotbar_slot: None,
            hotbar_drag: None,
            confirm_hotbar_reset: false,
            io_label: String::new(),
        }
    }
}

mod canvas;
mod challenge;
mod geometry;
mod hotbar;
mod render;
mod simulation;

use geometry::*;
use hotbar::*;
use simulation::*;

#[cfg(test)]
use challenge::*;

impl LogicEditor {
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
        self.set_context_hotbar_folder(("Component", default_component_slots()));
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();

        let hotbar_width = if self.active_hotbar_folder.is_empty() {
            HOTBAR_COLUMN_WIDTH + SIDE_PANEL_HORIZONTAL_MARGIN
        } else {
            HOTBAR_WIDTH
        };
        egui::Panel::left("logic-hotbar")
            .resizable(false)
            .exact_size(hotbar_width)
            .show_inside(ui, |ui| {
                self.show_hotbar(ui);
            });

        egui::Panel::left("logic-tool-settings")
            .resizable(false)
            .exact_size(HOTBAR_COLUMN_WIDTH + SIDE_PANEL_HORIZONTAL_MARGIN)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_tool_settings(ui);
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
            if self.gesture.is_some()
                || self.active_hotbar_slot.is_some()
                || self.tool.kind != ToolKind::Select
            {
                self.select_tool();
            } else {
                self.active_hotbar_folder.pop();
            }
        }

        context.request_repaint();
    }
}

#[cfg(test)]
mod tests;
