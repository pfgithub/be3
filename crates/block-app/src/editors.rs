#[cfg(not(target_os = "android"))]
mod browser_tab;
mod clipboard;
mod database;
mod database_schema;
pub(crate) mod image;
mod infinite_canvas;
mod pixel_art;
mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;

use block::BlockParent;
use block_client::{blocks::workspace_index::BlockEntry, BlockClient, BlockRelationships};
use eframe::egui;
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

use self::unsupported::UnsupportedEditor;

const COMPACT_DIRECT_EDITOR_WIDTH: f32 = 760.0;
const DIRECT_EDITOR_MIN_ZOOM: f32 = 0.25;
const DIRECT_EDITOR_MAX_ZOOM: f32 = 32.0;
const DIRECT_EDITOR_PAN_MARGIN: f32 = 32.0;

pub enum EditorAction {
    OpenBlock {
        id: Uuid,
        block_type: Uuid,
    },
    CreateBlock {
        block_type: Uuid,
        parent: Option<Uuid>,
    },
    SetParent {
        id: Uuid,
        parent: Uuid,
    },
}

pub struct BlockRenderContext<'a> {
    pub painter: &'a egui::Painter,
    pub corners: [egui::Pos2; 4],
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectEditorCapabilities {
    pub allow_rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub supports_pan_and_zoom: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum DirectEditorViewportCommand {
    Pan(egui::Vec2),
    Zoom {
        factor: f32,
        anchor: Option<egui::Pos2>,
    },
    Fit,
}

pub struct DirectEditorViewport {
    zoom: f32,
    commands: Vec<DirectEditorViewportCommand>,
}

impl DirectEditorViewport {
    pub fn new(zoom: f32) -> Self {
        Self {
            zoom,
            commands: Vec::new(),
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn pan(&mut self, delta: egui::Vec2) {
        self.commands.push(DirectEditorViewportCommand::Pan(delta));
    }

    pub fn change_zoom(&mut self, factor: f32, anchor: Option<egui::Pos2>) {
        self.commands
            .push(DirectEditorViewportCommand::Zoom { factor, anchor });
    }

    pub fn fit(&mut self) {
        self.commands.push(DirectEditorViewportCommand::Fit);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = DirectEditorViewportCommand> + '_ {
        self.commands.drain(..)
    }
}

pub struct EditorAccess<'a> {
    active: Uuid,
    client: &'a BlockClient,
    registry: &'a EditorRegistry,
    editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
}

impl<'a> EditorAccess<'a> {
    pub fn new(
        active: Uuid,
        client: &'a BlockClient,
        registry: &'a EditorRegistry,
        editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
    ) -> Self {
        Self {
            active,
            client,
            registry,
            editors,
        }
    }

    pub fn client(&self) -> &BlockClient {
        self.client
    }

    pub fn registry(&self) -> &EditorRegistry {
        self.registry
    }

    pub fn insert(&mut self, editor: Box<dyn BlockEditor>) {
        let id = editor.id();
        assert_ne!(id, self.active, "cannot replace the active editor");
        assert!(
            self.editors.insert(id, editor).is_none(),
            "editor {id} is already open"
        );
    }

    pub fn ensure(&mut self, id: Uuid, block_type: Uuid) {
        if id != self.active && !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(self.client, id, block_type));
        }
    }

    pub fn default_preserve_aspect_ratio(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.default_preserve_aspect_ratio())
    }

    pub fn render_aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.editors
            .get(&id)
            .and_then(|editor| editor.render_aspect_ratio())
    }

    pub fn render(&mut self, id: Uuid, context: BlockRenderContext<'_>) -> bool {
        let Some(mut editor) = self.editors.remove(&id) else {
            return false;
        };
        let rendered = editor.render(context);
        self.editors.insert(id, editor);
        rendered
    }

    pub fn direct_editor_capabilities(&self, id: Uuid) -> Option<DirectEditorCapabilities> {
        self.editors
            .get(&id)
            .and_then(|editor| editor.direct_editor_capabilities())
    }

    pub fn direct_editor_intrinsic_size(&mut self, id: Uuid) -> Option<egui::Vec2> {
        let mut editor = self.editors.remove(&id)?;
        let size = editor.direct_editor_intrinsic_size(self.client);
        self.editors.insert(id, editor);
        size
    }

    pub fn direct_editor_top_bar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let mut editor = self.editors.remove(&id)?;
        let action = editor.direct_editor_top_bar(ui, self.client, viewport);
        self.editors.insert(id, editor);
        action
    }

    pub fn direct_editor_has_left_sidebar(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.direct_editor_has_left_sidebar())
    }

    pub fn direct_editor_left_sidebar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
    ) -> Option<EditorAction> {
        let mut editor = self.editors.remove(&id)?;
        let action = editor.direct_editor_left_sidebar(ui, self.client);
        self.editors.insert(id, editor);
        action
    }

    pub fn direct_editor_has_right_sidebar(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.direct_editor_has_right_sidebar())
    }

    pub fn direct_editor_right_sidebar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
    ) -> Option<EditorAction> {
        let mut editor = self.editors.remove(&id)?;
        let action = editor.direct_editor_right_sidebar(ui, self.client);
        self.editors.insert(id, editor);
        action
    }

    pub fn direct_editor_ui(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let mut editor = self.editors.remove(&id)?;
        let action = editor.direct_editor_ui(ui, self.client, scale, viewport);
        self.editors.insert(id, editor);
        action
    }
}

#[derive(Clone)]
pub struct SidebarDragPayload {
    pub reference: block::BlockReference,
    pub source: SidebarDragSource,
    pub is_reference: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarDragSource {
    Root,
    Orphaned,
    Block(Uuid),
}

pub trait BlockEditor {
    fn id(&self) -> Uuid;
    fn block_type(&self) -> Uuid;
    fn name(&self) -> String;
    fn relationships(&self) -> Option<BlockRelationships>;
    fn set_parent(&self, parent: BlockParent);
    fn note_backref(&self, id: Uuid);
    fn add_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn delete_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn block_created(
        &mut self,
        _id: Uuid,
        _block_type: Uuid,
        _author: Uuid,
        _name: String,
    ) -> bool {
        false
    }
    fn update_open_tab(&mut self, _frame: &eframe::Frame) {}
    fn set_tab_active(&mut self, _active: bool) {}
    fn tab_closed(&mut self) {}
    fn history(&self) -> Option<&dyn block_client::BlockHistoryHandle> {
        None
    }
    fn render(&mut self, _context: BlockRenderContext<'_>) -> bool {
        false
    }
    fn render_aspect_ratio(&self) -> Option<f32> {
        None
    }
    fn default_preserve_aspect_ratio(&self) -> bool {
        false
    }
    fn direct_editor_capabilities(&self) -> Option<DirectEditorCapabilities> {
        None
    }
    fn direct_editor_intrinsic_size(&mut self, _client: &BlockClient) -> Option<egui::Vec2> {
        None
    }
    fn direct_editor_top_bar(
        &mut self,
        _ui: &mut egui::Ui,
        _client: &BlockClient,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_has_left_sidebar(&self) -> bool {
        false
    }
    fn direct_editor_left_sidebar(
        &mut self,
        _ui: &mut egui::Ui,
        _client: &BlockClient,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_has_right_sidebar(&self) -> bool {
        false
    }
    fn direct_editor_right_sidebar(
        &mut self,
        _ui: &mut egui::Ui,
        _client: &BlockClient,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_ui(
        &mut self,
        _ui: &mut egui::Ui,
        _client: &BlockClient,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        None
    }
    fn ui(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        None
    }
}

pub fn direct_editor_tab_ui(
    editor: &mut dyn BlockEditor,
    ui: &mut egui::Ui,
    client: &BlockClient,
) -> Option<EditorAction> {
    let id = editor.id();
    let compact = ui.available_width() < COMPACT_DIRECT_EDITOR_WIDTH;
    let mut action = None;
    let capabilities = editor.direct_editor_capabilities().unwrap();
    let viewport_id = egui::Id::new(("direct-editor-tab-viewport", id));
    let mut viewport_state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<DirectEditorTabViewport>(viewport_id))
        .unwrap_or_default();
    let mut viewport = DirectEditorViewport::new(viewport_state.zoom);

    egui::Panel::top(egui::Id::new(("direct-editor-tab-toolbar", id)))
        .show_separator_line(true)
        .show_inside(ui, |ui| {
            let next_action = editor.direct_editor_top_bar(ui, client, &mut viewport);
            if action.is_none() {
                action = next_action;
            }
        });

    if compact {
        let available = ui.available_rect_before_wrap();
        if editor.direct_editor_has_left_sidebar() {
            egui::Window::new("Left sidebar")
                .id(egui::Id::new(("direct-editor-tab-left-window", id)))
                .default_width(240.0)
                .default_pos(available.left_top() + egui::vec2(16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    let next_action = editor.direct_editor_left_sidebar(ui, client);
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
        if editor.direct_editor_has_right_sidebar() {
            egui::Window::new("Right sidebar")
                .id(egui::Id::new(("direct-editor-tab-right-window", id)))
                .pivot(egui::Align2::RIGHT_TOP)
                .default_width(240.0)
                .default_pos(available.right_top() + egui::vec2(-16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    let next_action = editor.direct_editor_right_sidebar(ui, client);
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
    } else {
        if editor.direct_editor_has_left_sidebar() {
            egui::Panel::left(egui::Id::new(("direct-editor-tab-left", id)))
                .default_size(240.0)
                .min_size(200.0)
                .max_size(340.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let next_action = editor.direct_editor_left_sidebar(ui, client);
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
        if editor.direct_editor_has_right_sidebar() {
            egui::Panel::right(egui::Id::new(("direct-editor-tab-right", id)))
                .default_size(240.0)
                .min_size(200.0)
                .max_size(340.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let next_action = editor.direct_editor_right_sidebar(ui, client);
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
    }

    let viewport_size = ui.available_size().max(egui::Vec2::splat(1.0));
    let intrinsic_size = editor
        .direct_editor_intrinsic_size(client)
        .unwrap_or_default();
    let content_size = egui::vec2(
        viewport_size.x.max(intrinsic_size.x),
        viewport_size.y.max(intrinsic_size.y),
    );
    if capabilities.supports_pan_and_zoom {
        let (viewport_rect, _) = ui.allocate_exact_size(viewport_size, egui::Sense::hover());
        let transformed_size = content_size * viewport_state.zoom;
        constrain_direct_editor_pan(&mut viewport_state.pan, viewport_size, transformed_size);
        let content_rect = egui::Rect::from_center_size(
            viewport_rect.center() + viewport_state.pan,
            transformed_size,
        );
        let next_action = ui
            .new_child(
                egui::UiBuilder::new()
                    .id_salt(("direct-editor-tab-content", id))
                    .max_rect(content_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .scope(|ui| {
                ui.set_clip_rect(viewport_rect.intersect(ui.clip_rect()));
                ui.set_min_size(transformed_size);
                editor.direct_editor_ui(ui, client, viewport_state.zoom, &mut viewport)
            })
            .inner;
        if action.is_none() {
            action = next_action;
        }

        for command in viewport.drain() {
            match command {
                DirectEditorViewportCommand::Pan(delta) => viewport_state.pan += delta,
                DirectEditorViewportCommand::Zoom { factor, anchor } => {
                    let old_zoom = viewport_state.zoom;
                    let new_zoom =
                        (old_zoom * factor).clamp(DIRECT_EDITOR_MIN_ZOOM, DIRECT_EDITOR_MAX_ZOOM);
                    if new_zoom != old_zoom {
                        let anchor = anchor.unwrap_or_else(|| viewport_rect.center());
                        viewport_state.pan = (anchor - viewport_rect.center())
                            - ((anchor - viewport_rect.center()) - viewport_state.pan)
                                * (new_zoom / old_zoom);
                        viewport_state.zoom = new_zoom;
                    }
                }
                DirectEditorViewportCommand::Fit => {
                    viewport_state = DirectEditorTabViewport::default();
                }
            }
        }
        constrain_direct_editor_pan(
            &mut viewport_state.pan,
            viewport_size,
            content_size * viewport_state.zoom,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(viewport_id, viewport_state));
    } else {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_size(content_size);
                let next_action = editor.direct_editor_ui(ui, client, 1.0, &mut viewport);
                if action.is_none() {
                    action = next_action;
                }
            });
    }

    action
}

#[derive(Clone, Copy, Debug)]
struct DirectEditorTabViewport {
    zoom: f32,
    pan: egui::Vec2,
}

impl Default for DirectEditorTabViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
        }
    }
}

fn constrain_direct_editor_pan(pan: &mut egui::Vec2, viewport: egui::Vec2, content: egui::Vec2) {
    for (pan, viewport, content) in [
        (&mut pan.x, viewport.x, content.x),
        (&mut pan.y, viewport.y, content.y),
    ] {
        if content <= viewport {
            *pan = 0.0;
        } else {
            let limit = (content - viewport) * 0.5 + DIRECT_EDITOR_PAN_MARGIN.min(viewport * 0.5);
            *pan = pan.clamp(-limit, limit);
        }
    }
}

type CreateEditor = fn(&BlockClient) -> Box<dyn BlockEditor>;
type OpenEditor = fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>;
type RegenerateDynamicArtifact =
    fn(&BlockClient, Uuid, Uuid, &[u8]) -> Result<Box<dyn DynamicArtifactRegeneration>, String>;

pub trait DynamicArtifactRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>>;
}

struct EditorRegistration {
    block_type: Uuid,
    display_name: &'static str,
    icon: MaterialIcon,
    create: Option<CreateEditor>,
    open: OpenEditor,
    can_add_child: bool,
    can_delete_child: bool,
    regenerate_dynamic_artifact: Option<RegenerateDynamicArtifact>,
}

impl EditorRegistration {
    fn regenerate_dynamic_artifact(
        &self,
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
        let regenerate = self.regenerate_dynamic_artifact.ok_or_else(|| {
            format!(
                "{} blocks do not support dynamic artifact regeneration",
                self.display_name
            )
        })?;
        regenerate(client, target_id, target_type, data)
    }
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    new_block_actions: Vec<(&'static str, Uuid)>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            new_block_actions: Vec::new(),
        };
        registry.register(database::registration());
        registry.register(database_schema::registration());
        registry.register(image::registration());
        registry.register(infinite_canvas::registration());
        registry.register(pixel_art::registration());
        registry.register(text::registration());
        #[cfg(not(target_os = "android"))]
        registry.register(browser_tab::registration());
        registry.register(workspace_index::registration());
        registry
    }

    fn register(&mut self, registration: EditorRegistration) {
        if registration.create.is_some() {
            self.new_block_actions
                .push((registration.display_name, registration.block_type));
        }
        self.registrations
            .insert(registration.block_type, registration);
    }

    pub fn new_block_actions(&self) -> &[(&'static str, Uuid)] {
        &self.new_block_actions
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn icon(&self, block_type: Uuid) -> Option<MaterialIcon> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.icon)
    }

    pub fn icon_label(&self, block_type: Uuid, label: &str) -> String {
        self.icon(block_type).map_or_else(
            || label.to_owned(),
            |icon| format!("{} {label}", icon.codepoint),
        )
    }

    pub fn can_add_child(&self, block_type: Uuid) -> bool {
        self.registrations
            .get(&block_type)
            .is_some_and(|registration| registration.can_add_child)
    }

    pub fn can_delete_child(&self, block_type: Uuid) -> bool {
        self.registrations
            .get(&block_type)
            .is_some_and(|registration| registration.can_delete_child)
    }

    pub fn regenerate_dynamic_artifact(
        &self,
        source_type: Uuid,
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        registration.regenerate_dynamic_artifact(client, target_id, target_type, data)
    }

    pub fn create(&self, client: &BlockClient, block_type: Uuid) -> Option<Box<dyn BlockEditor>> {
        self.registrations
            .get(&block_type)
            .and_then(|registration| registration.create.map(|create| create(client)))
    }

    pub fn open(&self, client: &BlockClient, id: Uuid, block_type: Uuid) -> Box<dyn BlockEditor> {
        self.registrations.get(&block_type).map_or_else(
            || Box::new(UnsupportedEditor::new(id, block_type)) as Box<dyn BlockEditor>,
            |registration| (registration.open)(client, id),
        )
    }
}
