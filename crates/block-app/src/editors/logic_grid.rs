mod canvas;
mod challenge;
pub(super) mod dynamic_artifact;
mod geometry;
mod hotbar;
mod render;
pub(super) mod renderer;
mod simulation;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};

use block::Block;
use block_client::{
    blocks::{
        compiled_logic::CompiledLogic,
        hotbar::{Hotbar, HotbarOperation, HotbarSlot as BlockHotbarSlot},
        logic_grid::{LogicGrid, LogicGridOperation},
    },
    BlockClient, BlockHandle,
};
use eframe::egui::{self, PointerButton};
use egui_material_icons::{
    icons::{ICON_BUILD, ICON_MEDIATION},
    MaterialIcon,
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

use self::renderer::{
    DrawRay, DrawStub, DrawTriangle, DrawValueTriangle, DrawWire, GridCallback, RenderFrame,
    WireValue,
};
use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport,
    DynamicArtifactSupport, EditorAccess, EditorAction, EditorKind,
};

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
    /// to place is looked up via `LogicGridEditor::active_hotbar_slot`.
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

/// A slot in the hotbar: either one of the built-in tools or a component
/// pinned from a compiled logic block.
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
        /// What to place. `None` until the compiled block has loaded, which is
        /// when the slot becomes usable.
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

pub(super) struct LogicGridEditor {
    block: BlockHandle<LogicGrid>,
    /// A working copy of the block's circuit. Reads run against this rather
    /// than taking a lock on every lookup; it is refreshed whenever the block
    /// changes, including from this editor's own edits.
    grid: Grid,
    observed_revision: Option<u64>,
    /// The hotbar block backing the palette, when the grid names one.
    hotbar_block: Option<BlockHandle<Hotbar>>,
    /// Every compiled block reachable from this grid, by ID. Pinned components
    /// and called programs are both read from here.
    compiled: HashMap<Uuid, BlockHandle<CompiledLogic>>,
    tool: Tool,
    placement_orientation: ComponentOrientation,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
    configured_storage: Option<ComponentId>,
    simulation: Simulation,
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
    compile_error: Option<String>,
    /// Keeps the client that owns the block alive in tests, which run without
    /// a workspace to hold it for them.
    #[cfg(test)]
    test_client: Option<BlockClient>,
}

impl EditorKind for LogicGridEditor {
    type Block = LogicGrid;

    const DISPLAY_NAME: &'static str = "Logic Grid";
    const ICON: MaterialIcon = ICON_MEDIATION;

    fn open(_client: &BlockClient, block: BlockHandle<LogicGrid>) -> Self {
        Self::new(block)
    }

    fn dynamic_artifact() -> Option<DynamicArtifactSupport> {
        Some(dynamic_artifact::SUPPORT)
    }
}

impl CreatableEditor for LogicGridEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(LogicGrid::new()))
    }
}

impl LogicGridEditor {
    fn new(block: BlockHandle<LogicGrid>) -> Self {
        Self {
            block,
            grid: Grid::new(),
            observed_revision: None,
            hotbar_block: None,
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

    /// Sends an edit to the block and refreshes the working copy from it, so
    /// the mirror never drifts from what the block actually applied.
    fn edit(&mut self, operation: LogicGridOperation) {
        self.block.operate(operation);
        self.observed_revision = None;
        self.sync(None);
    }

    fn edit_all(&mut self, operations: impl IntoIterator<Item = LogicGridOperation>) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if operations.is_empty() {
            return;
        }
        self.block.operate_grouped(operations);
        self.observed_revision = None;
        self.sync(None);
    }

    /// Adds a component under a freshly allocated ID, and hands the ID back so
    /// the caller can keep working with what it just placed.
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

    /// Refreshes everything read out of the block: the circuit, the level it
    /// belongs to, the hotbar and the compiled components it can reach.
    fn sync(&mut self, client: Option<&BlockClient>) {
        let revision = self.block.revision();
        if self.observed_revision == Some(revision) {
            self.sync_hotbar(client);
            return;
        }
        let Some(block) = self.block.read() else {
            return;
        };
        self.grid = block.grid().clone();
        let hotbar_id = block.hotbar();
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
            if self.hotbar_block.as_ref().map(BlockHandle::id) != hotbar_id {
                self.hotbar_block = hotbar_id.map(|id| client.get_block::<Hotbar>(id));
            }
            for compiled in called {
                self.ensure_compiled(client, compiled);
            }
        }
        self.sync_hotbar(client);
    }

    /// Keeps a handle on `compiled` and on everything it calls, so a circuit
    /// can be linked once all of them have loaded.
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

    /// Compiles the grid into a component other grids can call. The compiled
    /// block is an artifact of this one, so it is rebuilt from here rather
    /// than edited on its own.
    fn compile(&mut self, client: &BlockClient) -> Option<EditorAction> {
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
                let name = dynamic_artifact::artifact_name(&self.block.name());
                child.set_name(name.clone());
                let id = child.id();
                // Pinning it here is what makes the component reachable: the
                // hotbar is the only place a grid can be placed from.
                self.compiled.insert(id, child);
                self.pin_component(name, id);
                self.compile_error = None;
                Some(EditorAction::OpenBlock {
                    id,
                    block_type: CompiledLogic::TYPE_ID,
                })
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
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            GridCallback { frame },
        ));
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

impl BlockEditor for LogicGridEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    /// The editor keeps its own camera, so the tab's pan and zoom are left out
    /// of the way.
    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.sync(Some(editors.client()));
        ui.set_min_width(HOTBAR_WIDTH);
        let settings_height = 190.0;
        let hotbar_height = (ui.available_height() - settings_height).max(160.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), hotbar_height), |ui| {
            self.show_hotbar(ui);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("logic-tool-settings")
            .max_height(settings_height)
            .show(ui, |ui| {
                self.show_tool_settings(ui);
            });
        None
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.sync(Some(editors.client()));
        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Compile", ICON_BUILD.codepoint))
                .on_hover_text("Build a component other grids can call")
                .clicked()
            {
                action = self.compile(editors.client());
            }
            if let Some(challenge) = &self.challenge {
                ui.separator();
                ui.strong(challenge.id.name());
            }
            let errors = self.grid.validate().len();
            if errors > 0 {
                ui.separator();
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("{errors} placement problems"),
                );
            }
            if let Some(error) = &self.compile_error {
                ui.separator();
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        action
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.sync(Some(editors.client()));
        // Passing the level is recorded on the grid, which is where the game
        // reads each level's progress back from.
        if self.take_challenge_passed() {
            self.edit(LogicGridOperation::SetCompleted { completed: true });
        }
        if self.block.read().is_none() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        }
        self.canvas_ui(ui);
        None
    }
}

/// Tests drive the editor without a workspace, so they keep the client that
/// owns the block alive here rather than threading it through every case.
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
        editor.sync(None);
        editor
    }

    /// Changes the circuit the way something outside the editor would, by
    /// rewriting the block rather than the working copy.
    fn seed<R>(&mut self, build: impl FnOnce(&mut Grid) -> R) -> R {
        let mut grid = self.grid.clone();
        let result = build(&mut grid);
        self.block.replace(LogicGrid::from_grid(grid));
        self.observed_revision = None;
        self.sync(None);
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
