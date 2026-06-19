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

fn default_hotbar() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Builtin(ToolKind::MergerSplitter),
        HotbarSlot::Folder {
            name: "Component".to_string(),
            slots: default_component_slots(),
        },
        HotbarSlot::Folder {
            name: "Logic".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Not),
                HotbarSlot::Locked {
                    name: "And gate".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Or gate".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Storage".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Storage),
                HotbarSlot::Builtin(ToolKind::ConfigureStorage),
                HotbarSlot::Locked {
                    name: "Register".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Memory".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Display".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Led),
                HotbarSlot::Locked {
                    name: "Seven Segment".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Organization".to_string(),
            slots: vec![
                HotbarSlot::Locked {
                    name: "Comment".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Pattern".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Group".to_string(),
                },
            ],
        },
    ]
}

fn default_component_slots() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Input),
        HotbarSlot::Builtin(ToolKind::Output),
    ]
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

impl LogicEditor {
    pub fn set_component_files(&mut self, component_files: Option<ComponentFiles>) {
        self.component_files = component_files;
    }

    fn select_tool(&mut self) {
        self.tool.kind = ToolKind::Select;
        self.active_hotbar_slot = None;
        self.gesture = None;
        self.configured_storage = None;
    }

    fn select_hotbar_path(&mut self, path: Vec<usize>) {
        let Some(slot) = get_hotbar_slot(&self.hotbar, &path) else {
            return;
        };
        match slot {
            HotbarSlot::Builtin(kind) => {
                self.tool.kind = *kind;
                self.active_hotbar_slot = Some(path);
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            HotbarSlot::Custom { .. } => {
                self.tool.kind = ToolKind::Custom;
                self.active_hotbar_slot = Some(path);
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            HotbarSlot::Folder { .. } => {
                self.active_hotbar_folder = path;
            }
            HotbarSlot::Locked { .. } => {}
        }
    }

    /// Rebuilds the hotbar from persisted entries. An empty save keeps the
    /// default tree so new saves do not need to write default data eagerly.
    pub fn set_hotbar(&mut self, slots: Vec<SaveHotbarSlot>) {
        self.hotbar = if slots.is_empty() {
            default_hotbar()
        } else {
            slots
                .into_iter()
                .filter_map(hotbar_slot_from_save)
                .collect()
        };
        self.set_context_hotbar_folder(
            self.challenge
                .as_ref()
                .map(|_| ("Challenge", default_component_slots()))
                .unwrap_or_else(|| ("Component", default_component_slots())),
        );
        if self.tool.kind == ToolKind::Custom {
            self.tool.kind = ToolKind::Select;
            self.active_hotbar_slot = None;
        }
        self.active_hotbar_folder.clear();
    }

    /// Appends a compiled custom component to the hotbar. If a slot for the same
    /// source already exists it is updated in place instead of duplicated.
    pub fn add_custom_hotbar_slot(
        &mut self,
        name: String,
        source: ComponentFileRef,
        kind: ComponentKind,
    ) {
        if let Some(slot) = find_custom_hotbar_slot_mut(&mut self.hotbar, source) {
            *slot = HotbarSlot::Custom { name, source, kind };
        } else {
            self.hotbar.push(HotbarSlot::Custom { name, source, kind });
        }
        self.persist_hotbar();
    }

    /// Unpins a custom hotbar slot, persisting the change and fixing up the
    /// selected custom path when needed.
    fn remove_hotbar_slot(&mut self, path: &[usize]) {
        let Some(HotbarSlot::Custom { source, .. }) = get_hotbar_slot(&self.hotbar, path) else {
            return;
        };
        let source = *source;
        if let Some(files) = &self.component_files {
            if let Err(error) = files.remove_hotbar(source) {
                eprintln!("failed to unpin hotbar component: {error}");
            }
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.active_hotbar_slot = None;
            if self.tool.kind == ToolKind::Custom {
                self.select_tool();
            }
        }
    }

    fn remove_hotbar_folder(&mut self, path: &[usize]) {
        if !matches!(
            get_hotbar_slot(&self.hotbar, path),
            Some(HotbarSlot::Folder { .. })
        ) || hotbar_slot_contains_unremovable(get_hotbar_slot(&self.hotbar, path).unwrap())
        {
            return;
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.select_tool();
        }
        if self.active_hotbar_folder.starts_with(path) {
            self.active_hotbar_folder.clear();
        }
    }

    fn reset_hotbar(&mut self) {
        self.hotbar = default_hotbar();
        self.active_hotbar_folder.clear();
        self.select_tool();
        self.persist_hotbar();
    }

    fn persist_hotbar(&self) {
        let Some(files) = &self.component_files else {
            return;
        };
        if let Err(error) = files.save_hotbar(self.hotbar.iter().map(hotbar_slot_to_save).collect())
        {
            eprintln!("failed to save hotbar: {error}");
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
        self.set_context_hotbar_folder(("Component", default_component_slots()));
    }

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

    fn set_context_hotbar_folder(&mut self, (name, slots): (&str, Vec<HotbarSlot>)) {
        let Some(HotbarSlot::Folder {
            name: folder_name,
            slots: folder_slots,
            ..
        }) = self
            .hotbar
            .iter_mut()
            .find(|slot| matches!(slot, HotbarSlot::Folder { name, .. } if name == "Component" || name == "Challenge"))
        else {
            return;
        };
        *folder_name = name.to_string();
        *folder_slots = slots;
    }

    fn next_missing_challenge_input(&self) -> Option<(usize, Scale, String)> {
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

    fn next_missing_challenge_output(&self) -> Option<(usize, Scale, String)> {
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

    fn active_input_scale(&self) -> Scale {
        self.next_missing_challenge_input()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    fn active_output_scale(&self) -> Scale {
        self.next_missing_challenge_output()
            .map(|(_, scale, _)| scale)
            .unwrap_or(self.tool.scale)
    }

    fn active_tool_snap(&self) -> Scale {
        match self.tool.kind {
            ToolKind::Input => self.active_input_scale(),
            ToolKind::Output => self.active_output_scale(),
            _ => self.tool.snap(),
        }
    }

    fn add_input_at(&mut self, position: Point, rotation: Rotation) {
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

    fn add_output_at(&mut self, position: Point, rotation: Rotation) {
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

    fn show_hotbar(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<HotbarAction> = None;
        let mut remove: Option<Vec<usize>> = None;
        let mut remove_folder: Option<Vec<usize>> = None;
        let mut new_folder = false;
        let mut drop_target: Option<HotbarDropTarget> = None;
        let pointer_released = ui.ctx().input(|input| input.pointer.any_released());
        let dragging_hotbar = self.hotbar_drag.is_some();
        let dragged_slot = self
            .hotbar_drag
            .as_ref()
            .and_then(|path| get_hotbar_slot(&self.hotbar, path))
            .cloned();
        let rows = visible_hotbar_rows(&self.hotbar, &self.active_hotbar_folder);
        let key_entries = rows
            .iter()
            .rev()
            .find(|row| !row.entries.is_empty())
            .map(|row| row.entries.as_slice())
            .unwrap_or(&[]);

        if !ui.ctx().egui_wants_keyboard_input() {
            ui.ctx().input(|input| {
                for (index, key) in HOTBAR_KEYS.iter().enumerate() {
                    if input.key_pressed(*key) {
                        if let Some((path, _)) = key_entries.get(index) {
                            action = Some(HotbarAction::SelectPath(path.clone()));
                        }
                    }
                }
                if input.key_pressed(egui::Key::Backtick) {
                    self.active_hotbar_folder.pop();
                }
            });
        }

        let current_folder = self.active_hotbar_folder.clone();
        let hotbar_header = ui.horizontal(|ui| {
            let up_response = ui.add_enabled(
                !self.active_hotbar_folder.is_empty(),
                egui::Button::new("Up"),
            );
            if up_response.clicked() {
                self.active_hotbar_folder.pop();
            }
            if up_response.hovered() && pointer_released {
                let mut parent = current_folder.clone();
                parent.pop();
                drop_target = Some(HotbarDropTarget::Folder(parent));
            }
            if !self.active_hotbar_folder.is_empty() {
                ui.small(hotbar_folder_name(&self.hotbar, &self.active_hotbar_folder));
            } else {
                ui.small("Hotbar");
            }
        });
        if hotbar_header.response.hovered() && pointer_released {
            drop_target = Some(HotbarDropTarget::Folder(current_folder));
        }
        if dragging_hotbar && hotbar_header.response.hovered() {
            ui.painter().rect_stroke(
                hotbar_header.response.rect.expand(2.0),
                4.0,
                egui::Stroke::new(1.5, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }

        ui.menu_button("+", |ui| {
            if ui.button("New folder").clicked() {
                new_folder = true;
                ui.close();
            }
            if ui.button("Reset hotbar").clicked() {
                self.confirm_hotbar_reset = true;
                ui.close();
            }
        });

        let visible_row_count = if self.active_hotbar_folder.is_empty() {
            1
        } else {
            rows.len()
        };
        let column_height = ui.available_height();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = HOTBAR_COLUMN_GAP;
            for (row_index, row) in rows.iter().enumerate().take(visible_row_count) {
                let mut hovered_hotbar_slot = false;
                let row_response = egui::Frame::group(ui.style())
                    .fill(if row.active {
                        ui.visuals().selection.bg_fill.gamma_multiply(0.28)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    })
                    .show(ui, |ui| {
                        let column_width = HOTBAR_COLUMN_WIDTH - 16.0;
                        ui.set_width(column_width);
                        ui.set_min_height(column_height);
                        ui.vertical_centered(|ui| {
                            let (label_rect, _) = ui.allocate_exact_size(
                                egui::vec2(column_width, 18.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().text(
                                label_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &row.title,
                                egui::FontId::proportional(11.0),
                                ui.visuals().weak_text_color(),
                            );
                            egui::ScrollArea::vertical()
                                .id_salt(("hotbar-row", row_index, row.folder_path.as_slice()))
                                .auto_shrink([false, false])
                                .max_height((column_height - 24.0).max(HOTBAR_SLOT_SIZE))
                                .show(ui, |ui| {
                                    ui.set_width(column_width);
                                    ui.vertical_centered(|ui| {
                                        if row.entries.is_empty() {
                                            ui.allocate_exact_size(
                                                egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
                                                egui::Sense::hover(),
                                            );
                                        }
                                        for (path, slot) in &row.entries {
                                            let hotkey = key_entries
                                                .iter()
                                                .position(|(key_path, _)| key_path == path)
                                                .and_then(hotbar_key_label);
                                            let selected =
                                                self.active_hotbar_slot.as_ref() == Some(path);
                                            let open_folder = self.active_hotbar_folder == *path;
                                            let dragging = self.hotbar_drag.as_ref() == Some(path);
                                            let drop_highlight =
                                                self.hotbar_drag.as_ref().is_some_and(|source| {
                                                    source != path
                                                        && !path.starts_with(source)
                                                        && hotbar_slot_drop_target(
                                                            &self.hotbar,
                                                            path,
                                                        )
                                                        .is_some()
                                                });
                                            let response = hotbar_button(
                                                ui,
                                                selected,
                                                open_folder,
                                                dragging,
                                                drop_highlight,
                                                slot.label(),
                                                hotkey,
                                                |painter, rect| {
                                                    paint_slot_preview(painter, rect, slot);
                                                },
                                            );
                                            if response.clicked() {
                                                action =
                                                    Some(HotbarAction::SelectPath(path.clone()));
                                            }
                                            if response.drag_started() {
                                                self.hotbar_drag = Some(path.clone());
                                            }
                                            if response.hovered() {
                                                hovered_hotbar_slot = true;
                                                if pointer_released {
                                                    drop_target =
                                                        hotbar_slot_drop_target(&self.hotbar, path);
                                                }
                                            }
                                            if matches!(
                                                slot,
                                                HotbarSlot::Custom { .. }
                                                    | HotbarSlot::Folder { .. }
                                            ) {
                                                response.context_menu(|ui| {
                                                    if matches!(slot, HotbarSlot::Custom { .. })
                                                        && ui.button("Remove from hotbar").clicked()
                                                    {
                                                        remove = Some(path.clone());
                                                        ui.close();
                                                    }
                                                    if matches!(slot, HotbarSlot::Folder { .. })
                                                        && ui.button("Open folder").clicked()
                                                    {
                                                        action = Some(HotbarAction::OpenFolder(
                                                            path.clone(),
                                                        ));
                                                        ui.close();
                                                    }
                                                    if matches!(slot, HotbarSlot::Folder { .. })
                                                        && !hotbar_slot_contains_unremovable(slot)
                                                        && ui.button("Remove folder").clicked()
                                                    {
                                                        remove_folder = Some(path.clone());
                                                        ui.close();
                                                    }
                                                });
                                            }
                                        }
                                    });
                                });
                        });
                    });
                if row_response.response.hovered() && pointer_released && !hovered_hotbar_slot {
                    drop_target = Some(HotbarDropTarget::Folder(row.folder_path.clone()));
                }
            }
        });

        if let Some(path) = remove {
            self.remove_hotbar_slot(&path);
        }
        if let Some(path) = remove_folder {
            self.remove_hotbar_folder(&path);
        }
        if new_folder {
            let slots = get_hotbar_slots_mut(&mut self.hotbar, &self.active_hotbar_folder);
            slots.push(HotbarSlot::Folder {
                name: "Folder".to_string(),
                slots: Vec::new(),
            });
            self.persist_hotbar();
        }
        if pointer_released {
            if let (Some(source), Some(target)) = (self.hotbar_drag.take(), drop_target) {
                match target {
                    HotbarDropTarget::Slot(target) => {
                        move_hotbar_slot(&mut self.hotbar, &source, &target);
                    }
                    HotbarDropTarget::Folder(target) => {
                        move_hotbar_slot_to_folder(&mut self.hotbar, &source, &target);
                    }
                }
                self.persist_hotbar();
                if self
                    .active_hotbar_slot
                    .as_ref()
                    .is_some_and(|active| active.starts_with(&source))
                {
                    self.select_tool();
                }
                if self.active_hotbar_folder.starts_with(&source) {
                    self.active_hotbar_folder.clear();
                }
            } else {
                self.hotbar_drag = None;
            }
        } else if let Some(slot) = dragged_slot.as_ref() {
            paint_hotbar_drag_preview(ui.ctx(), slot);
        }

        match action {
            Some(HotbarAction::SelectPath(path)) => {
                if self.active_hotbar_slot.as_ref() == Some(&path) {
                    self.select_tool();
                } else {
                    self.select_hotbar_path(path);
                }
            }
            Some(HotbarAction::OpenFolder(path)) => {
                if self.active_hotbar_folder == path {
                    self.active_hotbar_folder.pop();
                } else {
                    self.active_hotbar_folder = path;
                }
            }
            None => {}
        }

        self.show_hotbar_reset_confirmation(ui.ctx());
    }

    fn show_tool_settings(&mut self, ui: &mut egui::Ui) {
        // Scale controls. They have no effect on custom components for now.
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
        // is fixed by the assigned port, so the field is not offered.
        if matches!(self.tool.kind, ToolKind::Input | ToolKind::Output) && self.challenge.is_none()
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

    fn show_hotbar_reset_confirmation(&mut self, context: &egui::Context) {
        if !self.confirm_hotbar_reset {
            return;
        }

        let mut reset = false;
        let mut cancel = false;
        let response = egui::Modal::new("reset-hotbar-modal".into()).show(context, |ui| {
            ui.set_min_width(300.0);
            ui.heading("Reset hotbar?");
            ui.label("This will restore the default hotbar layout.");
            ui.horizontal(|ui| {
                reset = ui.button("Reset").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if response.should_close() || cancel {
            self.confirm_hotbar_reset = false;
        }
        if reset {
            self.confirm_hotbar_reset = false;
            self.reset_hotbar();
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
        BTreeMap<(ComponentId, ConnectionSlot), u32>,
        BTreeMap<ComponentId, u32>,
        BTreeMap<ComponentId, u32>,
        Vec<WireValue>,
    ) {
        let simulation_vm = self
            .simulation
            .snapshot
            .as_ref()
            .filter(|simulation_snapshot| *simulation_snapshot == snapshot)
            .and_then(|_| self.simulation.vm.as_ref());
        let root_memory = simulation_vm.map(Vm::root_memory).unwrap_or_default();

        let mut values = Vec::new();
        let mut indices = BTreeMap::new();
        let mut net_value: BTreeMap<usize, u32> = BTreeMap::new();
        let mut address = 0;
        for (node_index, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::WireNet { wires } = node else {
                continue;
            };
            let value_index = values.len() as u32;
            net_value.insert(node_index, value_index);
            values.push(WireValue::new(
                root_memory.get(address).copied().unwrap_or_default(),
            ));
            for wire in wires {
                indices.insert(*wire, value_index);
            }
            address += 1;
        }

        let mut component_indices: BTreeMap<ComponentId, u32> = BTreeMap::new();
        let mut connection_indices: BTreeMap<(ComponentId, ConnectionSlot), u32> = BTreeMap::new();
        for (i, node) in snapshot.graph.nodes.iter().enumerate() {
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
            for edge in &snapshot.graph.edges {
                let other = if edge.first == GraphNodeId(i) {
                    Some(edge.second.0)
                } else if edge.second == GraphNodeId(i) {
                    Some(edge.first.0)
                } else {
                    None
                };
                if let Some(vi) = other.and_then(|j| net_value.get(&j)) {
                    connection_indices.insert(
                        (
                            *component,
                            ConnectionSlot {
                                id: *slot,
                                direction: *direction,
                                side: *side,
                                start: *start,
                                end: *end,
                                scale: *scale,
                            },
                        ),
                        *vi,
                    );
                    component_indices.insert(*component, *vi);
                    break;
                }
            }
        }

        let mut storage_indices = BTreeMap::new();
        for (storage_index, component) in self
            .grid
            .components()
            .filter(|component| matches!(component.kind, ComponentKind::Storage { .. }))
            .enumerate()
        {
            let ComponentKind::Storage { value, .. } = component.kind else {
                continue;
            };
            let value = simulation_vm
                .and_then(|vm| vm.storage.get(storage_index).copied())
                .unwrap_or(value);
            let value_index = values.len() as u32;
            values.push(WireValue::new(value));
            storage_indices.insert(component.id, value_index);
        }

        if values.is_empty() {
            values.push(WireValue::new(0));
        }
        (
            indices,
            connection_indices,
            component_indices,
            storage_indices,
            values,
        )
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

        if self.tool.kind.places_component() && !response.ctx.egui_wants_keyboard_input() {
            response.ctx.input(|input| {
                if input.key_pressed(egui::Key::Q) {
                    self.placement_rotation = rotate_left(self.placement_rotation);
                }
                if input.key_pressed(egui::Key::E) {
                    self.placement_rotation = rotate_right(self.placement_rotation);
                }
                if input.key_pressed(egui::Key::OpenBracket) {
                    self.tool.scale = previous_scale(self.tool.scale);
                }
                if input.key_pressed(egui::Key::CloseBracket) {
                    self.tool.scale = next_scale(self.tool.scale);
                }
            });
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
        let snapped = snap_point(world, self.active_tool_snap());

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
                .then(|| self.active_hotbar_slot.as_deref())
                .flatten()
                .and_then(|path| get_hotbar_slot(&self.hotbar, path))
                .and_then(|slot| match slot {
                    HotbarSlot::Custom { kind, .. } => Some(kind.clone()),
                    _ => None,
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
                })
                .filter(|_| {
                    self.challenge.is_none() || self.next_missing_challenge_input().is_some()
                }),
                ToolKind::Output => Some(Gesture::Output {
                    anchor: snapped,
                    drag_start: world,
                })
                .filter(|_| {
                    self.challenge.is_none() || self.next_missing_challenge_output().is_some()
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
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Not,
                    );
                    let scale = self.tool.scale;
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, scale, ToolKind::Not),
                        rotation,
                        ComponentKind::Not { scale },
                    );
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::MergerSplitter,
                    );
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
                Some(Gesture::Led { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Led,
                    );
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, Scale::ONE, ToolKind::Led),
                        rotation,
                        ComponentKind::Led,
                    );
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Storage,
                    );
                    let scale = self.tool.scale;
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, scale, ToolKind::Storage),
                        rotation,
                        ComponentKind::Storage { scale, value: 0 },
                    );
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Input,
                    );
                    let scale = self.active_input_scale();
                    let position =
                        component_placement_position(anchor, rotation, scale, ToolKind::Input);
                    self.add_input_at(position, rotation);
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Output,
                    );
                    let scale = self.active_output_scale();
                    let position =
                        component_placement_position(anchor, rotation, scale, ToolKind::Output);
                    self.add_output_at(position, rotation);
                }
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Custom,
                    );
                    let position = subcomponent_placement_position(anchor, rotation, &kind);
                    self.grid.add_component(position, rotation, kind);
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

    fn selected_custom_kind(&self) -> Option<ComponentKind> {
        (self.tool.kind == ToolKind::Custom)
            .then_some(self.active_hotbar_slot.as_deref())
            .flatten()
            .and_then(|path| get_hotbar_slot(&self.hotbar, path))
            .and_then(|slot| match slot {
                HotbarSlot::Custom { kind, .. } => Some(kind.clone()),
                _ => None,
            })
    }

    fn placement_preview_component(&self, pointer: [f32; 2]) -> Option<Component> {
        if let Some(kind) = self.selected_custom_kind() {
            return component_preview(
                self.tool,
                snap_point(pointer, kind.snap()),
                self.placement_rotation,
                Some(&kind),
            );
        }
        let mut tool = self.tool;
        tool.scale = match self.tool.kind {
            ToolKind::Input => {
                if self.challenge.is_some() && self.next_missing_challenge_input().is_none() {
                    return None;
                }
                self.active_input_scale()
            }
            ToolKind::Output => {
                if self.challenge.is_some() && self.next_missing_challenge_output().is_none() {
                    return None;
                }
                self.active_output_scale()
            }
            _ => self.tool.scale,
        };
        component_preview(
            tool,
            snap_point(pointer, self.active_tool_snap()),
            self.placement_rotation,
            None,
        )
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
        let (
            wire_value_indices,
            connection_value_indices,
            component_value_indices,
            storage_value_indices,
            wire_values,
        ) = self.wire_value_indices(&snapshot);
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
        let mut value_triangles = Vec::new();
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
            for connection in component.connection_slots() {
                let value_index = connection_value_indices
                    .get(&(component.id, connection))
                    .copied()
                    .unwrap_or_default();
                value_triangles.push(DrawValueTriangle::connection_marker(
                    component,
                    connection,
                    connection.scale.get() as f32 * 0.4,
                    value_index,
                ));
            }
            if let Some(value_index) = storage_value_indices.get(&component.id).copied() {
                value_triangles.extend(DrawValueTriangle::storage_state(component, value_index));
            }
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
            wire_triangles.extend(DrawTriangle::wire_endpoints(*wire));
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
            let snapped = snap_point(pointer, self.active_tool_snap());
            match self.gesture.as_ref() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(*start, snapped, self.tool.scale) {
                        wire_triangles
                            .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Not,
                    );
                    if let Some(component) = component_preview(self.tool, *anchor, rotation, None) {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::MergerSplitter,
                    );
                    if let Some(component) = component_preview(self.tool, *anchor, rotation, None) {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Led,
                    );
                    if let Some(component) = component_preview(self.tool, *anchor, rotation, None) {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Storage,
                    );
                    if let Some(component) = component_preview(self.tool, *anchor, rotation, None) {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Input,
                    );
                    let mut tool = self.tool;
                    tool.scale = self.active_input_scale();
                    if let Some(component) = component_preview(tool, *anchor, rotation, None) {
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
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Output,
                    );
                    let mut tool = self.tool;
                    tool.scale = self.active_output_scale();
                    if let Some(component) = component_preview(tool, *anchor, rotation, None) {
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
                    let rotation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_rotation,
                        ToolKind::Custom,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, rotation, Some(kind))
                    {
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
                None => {
                    if let Some(component) = self.placement_preview_component(pointer) {
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
            value_triangles,
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
    SelectPath(Vec<usize>),
    OpenFolder(Vec<usize>),
}

#[derive(Debug, PartialEq, Eq)]
enum HotbarDropTarget {
    Slot(Vec<usize>),
    Folder(Vec<usize>),
}

fn hotbar_key_label(index: usize) -> Option<&'static str> {
    match index {
        0 => Some("1"),
        1 => Some("2"),
        2 => Some("3"),
        3 => Some("4"),
        4 => Some("5"),
        5 => Some("6"),
        6 => Some("7"),
        7 => Some("8"),
        8 => Some("9"),
        9 => Some("0"),
        _ => None,
    }
}

fn hotbar_slot_from_save(slot: SaveHotbarSlot) -> Option<HotbarSlot> {
    Some(match slot {
        SaveHotbarSlot::Builtin { tool } => HotbarSlot::Builtin(ToolKind::from_id(&tool)?),
        SaveHotbarSlot::Locked { name } => HotbarSlot::Locked { name },
        SaveHotbarSlot::Folder { name, slots } => HotbarSlot::Folder {
            name,
            slots: slots
                .into_iter()
                .filter_map(hotbar_slot_from_save)
                .collect(),
        },
        SaveHotbarSlot::Custom { name, kind } => HotbarSlot::Custom {
            name,
            source: hotbar_kind_source(&kind)?,
            kind,
        },
    })
}

fn hotbar_slot_to_save(slot: &HotbarSlot) -> SaveHotbarSlot {
    match slot {
        HotbarSlot::Builtin(kind) => SaveHotbarSlot::Builtin {
            tool: kind.id().to_string(),
        },
        HotbarSlot::Locked { name } => SaveHotbarSlot::Locked { name: name.clone() },
        HotbarSlot::Folder { name, slots } => SaveHotbarSlot::Folder {
            name: name.clone(),
            slots: slots.iter().map(hotbar_slot_to_save).collect(),
        },
        HotbarSlot::Custom { name, kind, .. } => SaveHotbarSlot::Custom {
            name: name.clone(),
            kind: kind.clone(),
        },
    }
}

fn hotbar_kind_source(kind: &ComponentKind) -> Option<ComponentFileRef> {
    match kind {
        ComponentKind::Subcomponent { source_file_id, .. } => Some(*source_file_id),
        _ => None,
    }
}

fn hotbar_slot_contains_unremovable(slot: &HotbarSlot) -> bool {
    match slot {
        HotbarSlot::Builtin(_) | HotbarSlot::Locked { .. } => true,
        HotbarSlot::Folder { slots, .. } => slots.iter().any(hotbar_slot_contains_unremovable),
        HotbarSlot::Custom { .. } => false,
    }
}

struct HotbarRow {
    folder_path: Vec<usize>,
    title: String,
    active: bool,
    entries: Vec<(Vec<usize>, HotbarSlot)>,
}

fn visible_hotbar_rows(slots: &[HotbarSlot], active_folder: &[usize]) -> [HotbarRow; 2] {
    let (first_path, second_path) = if active_folder.len() <= 1 {
        (
            Vec::new(),
            (!active_folder.is_empty()).then(|| active_folder.to_vec()),
        )
    } else {
        (
            active_folder[..active_folder.len() - 1].to_vec(),
            Some(active_folder.to_vec()),
        )
    };
    let first_active = second_path.is_none();
    [
        hotbar_row(slots, first_path, first_active),
        match second_path {
            Some(path) => hotbar_row(slots, path, true),
            None => HotbarRow {
                folder_path: Vec::new(),
                title: "Open folder".to_string(),
                active: false,
                entries: Vec::new(),
            },
        },
    ]
}

fn hotbar_row(slots: &[HotbarSlot], folder_path: Vec<usize>, active: bool) -> HotbarRow {
    let title = hotbar_folder_name(slots, &folder_path).to_string();
    let entries = visible_hotbar_entries(slots, &folder_path);
    HotbarRow {
        folder_path,
        title,
        active,
        entries,
    }
}

fn visible_hotbar_entries(
    slots: &[HotbarSlot],
    folder_path: &[usize],
) -> Vec<(Vec<usize>, HotbarSlot)> {
    let slots = get_hotbar_slots(slots, folder_path).unwrap_or(slots);
    slots
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, slot)| {
            let mut path = folder_path.to_vec();
            path.push(index);
            (path, slot)
        })
        .collect()
}

fn hotbar_folder_name<'a>(slots: &'a [HotbarSlot], folder_path: &[usize]) -> &'a str {
    match get_hotbar_slot(slots, folder_path) {
        Some(HotbarSlot::Folder { name, .. }) => name,
        _ => "Hotbar",
    }
}

fn get_hotbar_slots<'a>(
    slots: &'a [HotbarSlot],
    folder_path: &[usize],
) -> Option<&'a [HotbarSlot]> {
    match folder_path.split_first() {
        None => Some(slots),
        Some((index, rest)) => match slots.get(*index)? {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots(slots, rest),
            _ => None,
        },
    }
}

fn get_hotbar_slots_mut<'a>(
    slots: &'a mut Vec<HotbarSlot>,
    folder_path: &[usize],
) -> &'a mut Vec<HotbarSlot> {
    match folder_path.split_first() {
        None => slots,
        Some((index, rest)) => match slots
            .get_mut(*index)
            .expect("active hotbar folder path is valid")
        {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots_mut(slots, rest),
            _ => panic!("active hotbar folder path points to a non-folder"),
        },
    }
}

fn get_hotbar_slot<'a>(slots: &'a [HotbarSlot], path: &[usize]) -> Option<&'a HotbarSlot> {
    let (index, rest) = path.split_first()?;
    let slot = slots.get(*index)?;
    if rest.is_empty() {
        return Some(slot);
    }
    match slot {
        HotbarSlot::Folder { slots, .. } => get_hotbar_slot(slots, rest),
        _ => None,
    }
}

fn find_custom_hotbar_slot_mut(
    slots: &mut [HotbarSlot],
    source: ComponentFileRef,
) -> Option<&mut HotbarSlot> {
    for slot in slots {
        match slot {
            HotbarSlot::Custom {
                source: existing, ..
            } if *existing == source => return Some(slot),
            HotbarSlot::Folder { slots, .. } => {
                if let Some(found) = find_custom_hotbar_slot_mut(slots, source) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn remove_hotbar_slot_at(slots: &mut Vec<HotbarSlot>, path: &[usize]) -> Option<HotbarSlot> {
    let (index, parent_path) = path.split_last()?;
    let parent = get_hotbar_slots_mut(slots, parent_path);
    (*index < parent.len()).then(|| parent.remove(*index))
}

fn hotbar_slot_drop_target(slots: &[HotbarSlot], path: &[usize]) -> Option<HotbarDropTarget> {
    if matches!(
        get_hotbar_slot(slots, path),
        Some(HotbarSlot::Folder { .. })
    ) {
        None
    } else {
        Some(HotbarDropTarget::Slot(path.to_vec()))
    }
}

fn move_hotbar_slot_to_folder(
    slots: &mut Vec<HotbarSlot>,
    source: &[usize],
    folder_path: &[usize],
) {
    if source == folder_path || folder_path.starts_with(source) {
        return;
    }
    let Some(slot) = remove_hotbar_slot_at(slots, source) else {
        return;
    };

    let mut adjusted_folder = folder_path.to_vec();
    if !folder_path.is_empty()
        && source.len() == folder_path.len()
        && source[..source.len() - 1] == folder_path[..folder_path.len() - 1]
        && source[source.len() - 1] < folder_path[folder_path.len() - 1]
    {
        *adjusted_folder
            .last_mut()
            .expect("folder path is non-empty") -= 1;
    }

    get_hotbar_slots_mut(slots, &adjusted_folder).push(slot);
}

fn move_hotbar_slot(slots: &mut Vec<HotbarSlot>, source: &[usize], target: &[usize]) {
    if source == target || target.starts_with(source) {
        return;
    }
    let Some(slot) = remove_hotbar_slot_at(slots, source) else {
        return;
    };

    if target.is_empty() {
        slots.push(slot);
        return;
    }

    let mut adjusted_target = target.to_vec();
    if source.len() == target.len()
        && source[..source.len() - 1] == target[..target.len() - 1]
        && source[source.len() - 1] < target[target.len() - 1]
    {
        *adjusted_target
            .last_mut()
            .expect("target path is non-empty") -= 1;
    }

    let Some((index, parent_path)) = adjusted_target.split_last() else {
        return;
    };
    let parent = get_hotbar_slots_mut(slots, parent_path);
    parent.insert((*index).min(parent.len()), slot);
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
    open_folder: bool,
    dragging: bool,
    drop_target: bool,
    label: &str,
    hotkey: Option<&str>,
    paint_preview: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
        egui::Sense::click_and_drag(),
    );
    let visuals = ui.visuals();
    let (bg, stroke) = if dragging {
        (
            visuals.widgets.inactive.bg_fill.gamma_multiply(0.65),
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if drop_target && response.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            egui::Stroke::new(2.0, visuals.selection.stroke.color),
        )
    } else if selected {
        (
            visuals.selection.bg_fill,
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if open_folder {
        (
            visuals.widgets.active.bg_fill,
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
    let text_color = if dragging {
        visuals.weak_text_color()
    } else {
        visuals.text_color()
    };
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
    if let Some(hotkey) = hotkey {
        let badge_rect =
            egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(15.0, 15.0));
        painter.rect_filled(badge_rect, 3.0, visuals.extreme_bg_color);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            hotkey,
            egui::FontId::proportional(9.0),
            text_color,
        );
    }
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.0),
        text_color,
    );
    response.on_hover_text(label)
}

fn paint_hotbar_drag_preview(context: &egui::Context, slot: &HotbarSlot) {
    let Some(pointer) = context.pointer_hover_pos() else {
        return;
    };
    let rect = egui::Rect::from_min_size(
        pointer + egui::vec2(14.0, 14.0),
        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
    );
    let painter = context.debug_painter();
    painter.rect(
        rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(24, 28, 36, 224),
        egui::Stroke::new(1.5, egui::Color32::from_rgb(140, 190, 255)),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(4.0);
    let preview_rect =
        egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), inner.height() * 0.66));
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(inner.min.x, preview_rect.max.y),
        egui::pos2(inner.max.x, inner.max.y),
    );
    paint_slot_preview(&painter, preview_rect, slot);
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        slot.label(),
        egui::FontId::proportional(9.0),
        egui::Color32::from_rgb(232, 236, 245),
    );
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
        HotbarSlot::Locked { .. } => {
            paint_glyph_box(painter, rect, egui::Color32::DARK_GRAY);
            let lock_rect = glyph_inset(rect, 0.32, 0.42, 0.68, 0.78);
            painter.rect_filled(lock_rect, 1.5, egui::Color32::DARK_GRAY);
            painter.circle_stroke(
                glyph_point(rect, 0.5, 0.42),
                rect.width() * 0.13,
                egui::Stroke::new(2.0, egui::Color32::DARK_GRAY),
            );
        }
        HotbarSlot::Folder { .. } => {
            let folder = color32(DrawTriangle::HIGHLIGHT_COLOR);
            painter.rect_filled(glyph_inset(rect, 0.12, 0.3, 0.88, 0.78), 2.0, folder);
            painter.rect_filled(glyph_inset(rect, 0.18, 0.2, 0.52, 0.38), 2.0, folder);
        }
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

fn rotate_left(rotation: Rotation) -> Rotation {
    match rotation {
        Rotation::Up => Rotation::Left,
        Rotation::Right => Rotation::Up,
        Rotation::Down => Rotation::Right,
        Rotation::Left => Rotation::Down,
    }
}

fn rotate_right(rotation: Rotation) -> Rotation {
    match rotation {
        Rotation::Up => Rotation::Right,
        Rotation::Right => Rotation::Down,
        Rotation::Down => Rotation::Left,
        Rotation::Left => Rotation::Up,
    }
}

fn previous_scale(scale: Scale) -> Scale {
    let current = scale.get() as u8;
    let value = SCALES
        .iter()
        .copied()
        .rev()
        .find(|value| *value < current)
        .unwrap_or(current);
    Scale::new(value).expect("scale shortcut uses valid scale")
}

fn next_scale(scale: Scale) -> Scale {
    let current = scale.get() as u8;
    let value = SCALES
        .iter()
        .copied()
        .find(|value| *value > current)
        .unwrap_or(current);
    Scale::new(value).expect("scale shortcut uses valid scale")
}

fn placement_rotation(
    drag_start: [f32; 2],
    pointer: [f32; 2],
    selected: Rotation,
    kind: ToolKind,
) -> Rotation {
    match drag_rotation(drag_start, pointer) {
        Some(rotation) if kind == ToolKind::Input => rotation.flip(),
        Some(rotation) => rotation,
        None => selected,
    }
}

fn component_preview(
    tool: Tool,
    anchor: Point,
    rotation: Rotation,
    custom_kind: Option<&ComponentKind>,
) -> Option<Component> {
    let kind = match tool.kind {
        ToolKind::Select | ToolKind::Wire | ToolKind::ConfigureStorage => return None,
        ToolKind::Not => ComponentKind::Not { scale: tool.scale },
        ToolKind::MergerSplitter => {
            let (input_scale, output_scale) = tool.conversion_scales();
            ComponentKind::MergerSplitter {
                input_scale,
                output_scale,
            }
        }
        ToolKind::Led => ComponentKind::Led,
        ToolKind::Storage => ComponentKind::Storage {
            scale: tool.scale,
            value: 0,
        },
        ToolKind::Input => ComponentKind::Input {
            scale: tool.scale,
            id: InputId::from_u128(u128::MAX),
            label: String::new(),
        },
        ToolKind::Output => ComponentKind::Output {
            scale: tool.scale,
            id: OutputId::from_u128(u128::MAX),
            label: String::new(),
        },
        ToolKind::Custom => custom_kind.cloned()?,
    };
    let position = match tool.kind {
        ToolKind::Custom => subcomponent_placement_position(anchor, rotation, &kind),
        ToolKind::MergerSplitter => {
            let (_, output_scale) = tool.conversion_scales();
            component_placement_position(anchor, rotation, output_scale, tool.kind)
        }
        ToolKind::Led => component_placement_position(anchor, rotation, Scale::ONE, tool.kind),
        _ => component_placement_position(anchor, rotation, tool.scale, tool.kind),
    };
    Some(Component {
        id: ComponentId(u64::MAX),
        position,
        rotation,
        kind,
    })
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
