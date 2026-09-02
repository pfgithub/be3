mod app_impl;
mod canvas;
mod challenge;
mod dynamic_artifact;
mod geometry;
mod hotbar;
mod render;
mod simulation;

pub use app_impl::LogicGridApp;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};

use block::Block;
use block_client::root_settings::RootSetting;
use block_client::{
    block_ref::BlockRef,
    blocks::{
        compiled_logic::CompiledLogic,
        hotbar::{Hotbar, HotbarOperation, HotbarSlot as BlockHotbarSlot},
        logic_grid::{LogicGrid, LogicGridOperation},
    },
    BlockClient, BlockHandle,
};
use block_editor_plugin::{
    egui::{self, PointerButton},
    egui_material_icons::icons::ICON_BUILD,
    EditorHost,
};
use logicgame::{
    challenges::{generate_challenge, Challenge, ChallengeId},
    execution::{Component as ExecutionComponent, Instruction, Pc, Vm},
    grid::{
        value_mask, CircuitGraph, Component, ComponentId, ComponentKind, ComponentOrientation,
        ComponentSide, ConnectionSlot, GraphNode, GraphNodeId, InputId, LogicGrid as Grid,
        OutputId, Point, Rotation, Scale, ValidationError, Wire,
    },
};
use uuid::Uuid;

use crate::frame::{
    DrawRay, DrawStub, DrawTriangle, DrawValueTriangle, DrawWire, RenderFrame, WireValue,
};
use std::sync::Arc;

use geometry::*;
use hotbar::*;
use simulation::*;

#[cfg(test)]
use challenge::*;

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 768.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const HOTBAR_COLUMN_WIDTH: f32 = 92.0;
const HOTBAR_COLUMN_GAP: f32 = 8.0;
const SIDE_PANEL_HORIZONTAL_MARGIN: f32 = 16.0;
const HOTBAR_WIDTH: f32 =
    HOTBAR_COLUMN_WIDTH * 2.0 + HOTBAR_COLUMN_GAP + SIDE_PANEL_HORIZONTAL_MARGIN;

const LABEL_COLOR: egui::Color32 = egui::Color32::from_rgb(232, 236, 245);

const NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(232, 236, 245);

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
    Component {
        name: String,
        compiled: Uuid,

        kind: Option<ComponentKind>,
    },
}

impl HotbarSlot {
    fn label(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.label(),
            Self::Locked { name } | Self::Folder { name, .. } | Self::Component { name, .. } => {
                name
            }
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

    passed_event: bool,
}

#[derive(Debug, Default)]
struct ChallengeTest {
    snapshot: Option<SimulationSnapshot>,
    error: Option<String>,

    input_slots: Vec<Option<usize>>,

    output_slots: Vec<Option<usize>>,

    next_tick: usize,

    actual: Vec<Vec<u64>>,

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

pub(super) struct LogicGridEditor {
    block: BlockHandle<LogicGrid>,

    grid: Grid,
    observed_revision: Option<u64>,

    hotbar_block: Option<RootSetting<Hotbar>>,

    hotbar_needs_write: bool,

    compiled: HashMap<Uuid, BlockHandle<CompiledLogic>>,
    tool: Tool,
    placement_orientation: ComponentOrientation,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
    configured_storage: Option<ComponentId>,
    simulation: Simulation,
    challenge: Option<ChallengeState>,

    hotbar: Vec<HotbarSlot>,

    active_hotbar_folder: Vec<usize>,

    active_hotbar_slot: Option<Vec<usize>>,

    hotbar_drag: Option<Vec<usize>>,
    confirm_hotbar_reset: bool,

    io_label: String,
    compile_error: Option<String>,

    #[cfg(test)]
    test_client: Option<BlockClient>,
}

const DISPLAY_NAME: &str = "Logic Grid";

impl LogicGridEditor {
    fn new(block: BlockHandle<LogicGrid>) -> Self {
        Self {
            block,
            grid: Grid::new(),
            observed_revision: None,
            hotbar_block: None,
            hotbar_needs_write: false,
            compiled: HashMap::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
                merger_out_scale: Scale::new(4).expect("default scale is valid"),
            },
            placement_orientation: ComponentOrientation::Right,
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
            configured_storage: None,
            simulation: Simulation::default(),
            challenge: None,
            hotbar: default_hotbar(),
            active_hotbar_folder: Vec::new(),
            active_hotbar_slot: None,
            hotbar_drag: None,
            confirm_hotbar_reset: false,
            io_label: String::new(),
            compile_error: None,
            #[cfg(test)]
            test_client: None,
        }
    }

    fn edit(&mut self, operation: LogicGridOperation) {
        self.block.operate(operation);
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
    }

    fn edit_all(&mut self, operations: impl IntoIterator<Item = LogicGridOperation>) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if operations.is_empty() {
            return;
        }
        self.block.operate_grouped(operations);
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
    }

    fn place(
        &mut self,
        position: Point,
        orientation: ComponentOrientation,
        kind: ComponentKind,
    ) -> ComponentId {
        let id = self.grid.next_component_id();
        self.edit(LogicGridOperation::AddComponent {
            component: Component {
                id,
                position,
                orientation,
                kind,
            },
        });
        id
    }

    fn toggle_storage_bit(&mut self, id: ComponentId, bit: u32) {
        let Some(Component {
            kind: ComponentKind::Storage { scale, value },
            ..
        }) = self.grid.component(id)
        else {
            return;
        };
        if bit >= scale.get() as u32 {
            return;
        }
        let value = value ^ (1_u64 << bit);
        self.edit(LogicGridOperation::SetStorageValue { id, value });
    }

    fn sync(&mut self, client: Option<&BlockClient>, client_id: Uuid) {
        let revision = self.block.revision();
        if self.observed_revision == Some(revision) {
            self.sync_hotbar(client, client_id);
            return;
        }
        let Some(block) = self.block.read() else {
            return;
        };
        self.grid = block.grid().clone();
        let challenge = block.challenge();
        let called = block.called_blocks();
        drop(block);
        self.observed_revision = Some(revision);

        if self.challenge.as_ref().map(|state| state.id) != challenge {
            self.challenge = challenge.map(|id| ChallengeState {
                id,
                data: generate_challenge(id),
                test: ChallengeTest::default(),
                passed_event: false,
            });
        }
        if let Some(client) = client {
            for compiled in called {
                self.ensure_compiled(client, compiled);
            }
        }
        self.sync_hotbar(client, client_id);
    }

    fn ensure_compiled(&mut self, client: &BlockClient, compiled: Uuid) {
        if self.compiled.contains_key(&compiled) {
            return;
        }
        let handle = client.get_block::<CompiledLogic>(compiled);
        let calls = handle
            .read()
            .map(|program| program.calls().to_vec())
            .unwrap_or_default();
        self.compiled.insert(compiled, handle);
        for called in calls {
            self.ensure_compiled(client, called);
        }
    }

    fn compiled_kind(&self, compiled: Uuid, name: &str) -> Option<ComponentKind> {
        self.compiled
            .get(&compiled)?
            .read()?
            .placement(compiled, name)
            .ok()
    }

    fn compile(&mut self, client: &BlockClient) -> Option<(Uuid, Uuid)> {
        let compiled = self
            .block
            .read()
            .map(|grid| dynamic_artifact::generate_initial(self.block.id(), &grid))?;
        match compiled {
            Ok(compiled) => {
                let child = client.create_dynamic_artifact(
                    compiled,
                    dynamic_artifact::descriptor(self.block.id()),
                );
                let source_name = self.block.name().unwrap_or_else(|| DISPLAY_NAME.to_owned());
                let name = dynamic_artifact::artifact_name(&source_name);
                child.set_name(name.clone());
                let id = child.id();

                self.compiled.insert(id, child);
                self.pin_component(name, id);
                self.compile_error = None;
                Some((id, CompiledLogic::TYPE_ID))
            }
            Err(error) => {
                self.compile_error = Some(error);
                None
            }
        }
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.handle_canvas_input(&response);

        let pointer_world = response
            .hovered()
            .then(|| context.pointer_hover_pos())
            .flatten()
            .map(|position| self.camera.screen_to_world(position, response.rect));

        self.update_simulation_preview();
        self.show_metrics(&context);
        self.show_storage_configuration(&context);
        self.show_simulation(&context);
        self.show_challenge(&context);

        let hovered_square = pointer_world.map(|pointer| snap_point(pointer, self.tool.snap()));
        let hovered_entity = self.show_grid_debugger(&context, hovered_square);
        let graph_hover = self.show_generated_graph(&context);
        let frame = self.render_frame(response.rect, pointer_world, hovered_entity, &graph_hover);
        crate::paint::paint(&painter, response.rect, frame);
        self.draw_component_labels(&painter, response.rect);
        if let (Some(pointer), Some(Gesture::SelectBox { start, .. })) =
            (pointer_world, self.gesture.as_ref())
        {
            let start = world_to_screen(*start, self.camera, response.rect);
            let end = world_to_screen(pointer, self.camera, response.rect);
            let selection_rect = egui::Rect::from_two_pos(start, end);
            painter.rect_filled(
                selection_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(66, 153, 225, 28),
            );
            painter.rect_stroke(
                selection_rect,
                0.0,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 180, 255)),
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
impl LogicGridEditor {
    fn detached(grid: Grid, challenge: Option<ChallengeId>) -> Self {
        let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
        let mut block = LogicGrid::from_grid(grid);
        if let Some(challenge) = challenge {
            block = block.with_challenge(challenge);
        }
        let mut editor = Self::new(client.create_block(block));
        editor.test_client = Some(client);
        editor.sync(None, Uuid::nil());
        editor
    }

    fn seed<R>(&mut self, build: impl FnOnce(&mut Grid) -> R) -> R {
        let mut grid = self.grid.clone();
        let result = build(&mut grid);
        self.block.replace(LogicGrid::from_grid(grid));
        self.observed_revision = None;
        self.sync(None, Uuid::nil());
        result
    }
}

#[cfg(test)]
impl Default for LogicGridEditor {
    fn default() -> Self {
        Self::detached(Grid::new(), None)
    }
}

#[cfg(test)]
mod tests;
