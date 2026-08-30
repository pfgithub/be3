mod browser_tab;
mod clipboard;
pub(crate) mod deterministic_game;
pub(crate) mod infinite_canvas;
mod logic_grid;
mod map;
mod pixel_ray_tracer;
pub(crate) mod plugin;
mod scene_3d;
mod text;
mod unsupported;
mod version_control_worktree;
mod video;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use block::{Block, BlockAccess, BlockParent, BlockReference};
use block_client::{
    blocks::{self, image::Image, workspace_index::BlockEntry},
    BlockClient, BlockHandle, BlockHandleAccess, BlockHistoryHandle, BlockRelationships,
};
use block_plugin_api::PluginManifest;
pub(super) use block_ui::{name_galley, paint_name, BlockLabel};
use block_ui::{BlockTypeEntry, BlockTypes};
use eframe::egui;
use egui_material_icons::{icons::ICON_LOCK, MaterialIcon};
use uuid::Uuid;

use self::unsupported::UnsupportedEditor;
use crate::platform::{FileFilter, PickedFile};

const COMPACT_DIRECT_EDITOR_WIDTH: f32 = 760.0;
const DIRECT_EDITOR_MIN_ZOOM: f32 = 0.25;
const DIRECT_EDITOR_MAX_ZOOM: f32 = 32.0;
pub(super) const EMBEDDED_EDITOR_PADDING: f32 = 12.0;
pub(super) const EMBEDDED_EDITOR_TITLE_HEIGHT: f32 = 28.0;
pub(super) const EMBEDDED_EDITOR_TITLE_GAP: f32 = 8.0;

pub(super) fn embedded_editor_frame_size(intrinsic: egui::Vec2, scale: f32) -> egui::Vec2 {
    egui::vec2(
        (intrinsic.x + EMBEDDED_EDITOR_PADDING * 2.0) * scale,
        (intrinsic.y
            + EMBEDDED_EDITOR_PADDING * 2.0
            + EMBEDDED_EDITOR_TITLE_HEIGHT
            + EMBEDDED_EDITOR_TITLE_GAP)
            * scale,
    )
}

pub fn install_render_resources(creation_context: &eframe::CreationContext<'_>) {
    logic_grid::renderer::install(creation_context);
    scene_3d::renderer::install(creation_context);
}

pub enum EditorAction {
    OpenBlock { id: Uuid, block_type: Uuid },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorInteraction {
    Preview,
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorResize {
    None,
    Horizontal,
    Vertical,
    Both,
}

impl DirectEditorResize {
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub fn vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorViewportInput {
    Background,
    Viewport,
    Editor,
}

#[derive(Clone, Copy, Debug)]
pub enum DirectEditorViewportCommand {
    Pan(egui::Vec2),
    Zoom {
        factor: f32,
        anchor: Option<egui::Pos2>,
    },
    Fit,
    AutoFit(Uuid),
    ResumeAutoFit,
}

pub struct DirectEditorViewport {
    zoom: f32,
    commands: Vec<DirectEditorViewportCommand>,
    content_rect: Option<egui::Rect>,
}

impl DirectEditorViewport {
    pub fn new(zoom: f32) -> Self {
        Self {
            zoom,
            commands: Vec::new(),
            content_rect: None,
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn push(&mut self, command: DirectEditorViewportCommand) {
        self.commands.push(command);
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

    pub fn auto_fit(&mut self, target: Uuid) {
        self.commands
            .push(DirectEditorViewportCommand::AutoFit(target));
    }

    pub fn resume_auto_fit(&mut self) {
        self.commands
            .push(DirectEditorViewportCommand::ResumeAutoFit);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = DirectEditorViewportCommand> + '_ {
        self.commands.drain(..)
    }

    pub fn content_rect(&self) -> Option<egui::Rect> {
        self.content_rect
    }

    pub fn replace_content_rect(&mut self, rect: Option<egui::Rect>) -> Option<egui::Rect> {
        std::mem::replace(&mut self.content_rect, rect)
    }
}

pub fn editor_access_ceiling(client: &BlockClient, id: Uuid) -> BlockAccess {
    let access = client.block_access(id);
    if client.is_dynamic_artifact(id) {
        access.min(BlockAccess::View)
    } else {
        access
    }
}

fn editor_scope<R>(
    ui: &mut egui::Ui,
    read_only: bool,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if !read_only {
        return contents(ui);
    }
    let mut style = (**ui.style()).clone();
    style.visuals.disabled_alpha = 1.0;
    ui.scope_builder(egui::UiBuilder::new().style(style).disabled(), contents)
        .inner
}

fn no_access_notice(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.weak(format!("{} No access", ICON_LOCK.codepoint));
    });
}

fn rect_corners(rect: egui::Rect) -> [egui::Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

fn fit_rect(available: egui::Rect, ratio: f32) -> egui::Rect {
    let ratio = ratio.max(0.01);
    let available_ratio = available.width() / available.height().max(1.0);
    let size = if available_ratio > ratio {
        egui::Vec2::new(available.height() * ratio, available.height())
    } else {
        egui::Vec2::new(available.width(), available.width() / ratio)
    };
    egui::Rect::from_center_size(available.center(), size)
}

fn paint_block_fallback(
    painter: &egui::Painter,
    rect: egui::Rect,
    reference: Option<&BlockReference>,
    editors: &EditorAccess<'_>,
) {
    painter.rect_filled(rect, 5.0, egui::Color32::from_gray(28));
    painter.rect_stroke(
        rect,
        5.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(75)),
        egui::StrokeKind::Inside,
    );
    let label = reference.map(|reference| BlockLabel::for_reference(editors.registry(), reference));
    let center = rect.center();
    if let Some(icon) = label.as_ref().and_then(|label| label.icon) {
        painter.text(
            center - egui::Vec2::new(0.0, 18.0),
            egui::Align2::CENTER_CENTER,
            icon.codepoint,
            egui::FontId::new(28.0, icon.font_family()),
            egui::Color32::LIGHT_GRAY,
        );
    }
    let (name, automatic) = label.as_ref().map_or(("Loading…", false), |label| {
        (label.name.as_str(), label.automatic)
    });
    paint_name(
        painter,
        center + egui::Vec2::new(0.0, 18.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(16.0),
        egui::Color32::LIGHT_GRAY,
        automatic,
    );
}

pub struct EditorAccess<'a> {
    active: Vec<Uuid>,
    access: BlockAccess,
    client: &'a Arc<BlockClient>,
    client_id: Uuid,
    registry: &'a EditorRegistry,
    editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
}

pub fn embedded_editor_ui(
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    id_salt: impl Hash,
    rect: egui::Rect,
    clip_rect: egui::Rect,
    scale: f32,
    viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let input = editors.direct_editor_viewport_input(block_id);
    let previous = viewport.replace_content_rect(Some(rect));
    let action = ui
        .new_child(
            egui::UiBuilder::new()
                .id_salt(id_salt)
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
        .scope(|ui| {
            ui.set_clip_rect(clip_rect.intersect(ui.clip_rect()));
            ui.set_max_size(rect.size());
            ui.set_min_size(rect.size());
            editors.embedded_direct_editor_ui(block_id, ui, scale, viewport)
        })
        .inner;
    viewport.replace_content_rect(previous);
    if input == DirectEditorViewportInput::Viewport {
        viewport_gesture_input(ui.ctx(), rect.intersect(clip_rect), None, viewport);
    }
    action
}

impl<'a> EditorAccess<'a> {
    pub fn new(
        active: Uuid,
        access: BlockAccess,
        client: &'a Arc<BlockClient>,
        client_id: Uuid,
        registry: &'a EditorRegistry,
        editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
    ) -> Self {
        Self {
            active: vec![active],
            access,
            client,
            client_id,
            registry,
            editors,
        }
    }

    pub fn access(&self) -> BlockAccess {
        self.access
    }

    fn access_for(&self, id: Uuid) -> BlockAccess {
        self.access.min(editor_access_ceiling(self.client, id))
    }

    pub fn client(&self) -> &BlockClient {
        self.client
    }

    pub fn client_handle(&self) -> Arc<BlockClient> {
        Arc::clone(self.client)
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id
    }

    pub fn registry(&self) -> &EditorRegistry {
        self.registry
    }

    pub fn insert(&mut self, editor: Box<dyn BlockEditor>) {
        let id = editor.id();
        assert!(
            !self.active.contains(&id),
            "cannot replace an active editor"
        );
        assert!(
            self.editors.insert(id, editor).is_none(),
            "editor {id} is already open"
        );
    }

    pub fn is_open(&self, id: Uuid) -> bool {
        self.editors.contains_key(&id)
    }

    pub fn ensure(&mut self, id: Uuid, block_type: Uuid) {
        if !self.active.contains(&id) && !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(self.client, id, block_type));
        }
    }

    fn with_editor<T>(
        &mut self,
        id: Uuid,
        callback: impl FnOnce(&mut dyn BlockEditor, &mut Self) -> T,
    ) -> Option<T> {
        let mut editor = self.editors.remove(&id)?;
        let nested = self.access_for(id);
        let access = std::mem::replace(&mut self.access, nested);
        self.active.push(id);
        let result = callback(editor.as_mut(), self);
        assert_eq!(self.active.pop(), Some(id));
        self.access = access;
        self.editors.insert(id, editor);
        Some(result)
    }

    fn with_editor_ui<T>(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        callback: impl FnOnce(&mut dyn BlockEditor, &mut Self, &mut egui::Ui) -> T,
    ) -> Option<T> {
        let access = self.access_for(id);
        if !access.can_view() {
            no_access_notice(ui);
            return None;
        }
        editor_scope(ui, !access.can_edit(), |ui| {
            self.with_editor(id, |editor, editors| callback(editor, editors, ui))
        })
    }

    pub fn default_preserve_aspect_ratio(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.default_preserve_aspect_ratio())
    }

    pub fn preview_aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.editors
            .get(&id)
            .and_then(|editor| editor.render_aspect_ratio())
    }

    pub fn render(&mut self, id: Uuid, context: BlockRenderContext<'_>) -> bool {
        if !self.access_for(id).can_view() {
            return false;
        }
        self.with_editor(id, |editor, editors| editor.render(context, editors))
            .unwrap_or(false)
    }

    pub fn direct_editor_capabilities(&self, id: Uuid) -> Option<DirectEditorCapabilities> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_capabilities())
    }

    pub fn direct_editor_interaction(&self, id: Uuid) -> Option<DirectEditorInteraction> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_interaction())
    }

    pub fn direct_editor_resize(&self, id: Uuid) -> Option<DirectEditorResize> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_resize())
    }

    pub fn direct_editor_viewport_input(&self, id: Uuid) -> DirectEditorViewportInput {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_viewport_input(self))
            .unwrap_or(DirectEditorViewportInput::Background)
    }

    pub fn direct_editor_intrinsic_size(&mut self, id: Uuid) -> Option<egui::Vec2> {
        self.with_editor(id, |editor, editors| {
            editor.direct_editor_intrinsic_size(editors)
        })?
    }

    pub fn direct_editor_intrinsic_size_for_width(
        &mut self,
        id: Uuid,
        width: f32,
    ) -> Option<egui::Vec2> {
        self.with_editor(id, |editor, editors| {
            editor.direct_editor_intrinsic_size_for_width(width, editors)
        })?
    }

    pub fn set_direct_editor_intrinsic_size(&mut self, id: Uuid, size: egui::Vec2) -> bool {
        if !self.access_for(id).can_edit() {
            return false;
        }
        self.with_editor(id, |editor, editors| {
            editor.set_direct_editor_intrinsic_size(size, editors)
        })
        .unwrap_or(false)
    }

    pub fn direct_editor_top_bar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.direct_editor_top_bar(ui, editors, viewport)
        })?
    }

    pub fn direct_editor_has_left_sidebar(&mut self, id: Uuid) -> bool {
        self.with_editor(id, |editor, editors| {
            editor.direct_editor_has_left_sidebar(editors)
        })
        .unwrap_or(false)
    }

    pub fn direct_editor_left_sidebar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.direct_editor_left_sidebar(ui, editors)
        })?
    }

    pub fn direct_editor_has_right_sidebar(&mut self, id: Uuid) -> bool {
        self.with_editor(id, |editor, editors| {
            editor.direct_editor_has_right_sidebar(editors)
        })
        .unwrap_or(false)
    }

    pub fn direct_editor_right_sidebar(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.direct_editor_right_sidebar(ui, editors)
        })?
    }

    pub fn embedded_direct_editor_ui(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.embedded_direct_editor_ui(ui, editors, scale, viewport)
        })?
    }

    pub fn set_parent(&mut self, id: Uuid, parent: BlockParent) -> bool {
        if !self.access_for(id).can_edit() {
            return false;
        }
        self.with_editor(id, |editor, _| editor.set_parent(parent))
            .is_some()
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
    fn block(&self) -> &dyn BlockHandleAccess;
    fn id(&self) -> Uuid {
        self.block().id()
    }
    fn block_type(&self) -> Uuid {
        self.block().block_type()
    }
    fn name(&self) -> Option<String> {
        self.block().name()
    }
    fn relationships(&self) -> Option<BlockRelationships> {
        self.block().relationships()
    }
    fn set_parent(&self, parent: BlockParent) {
        self.block().set_parent(parent);
    }
    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        self.block().add_child(entry.id)
    }
    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        self.block().delete_child(entry.id)
    }
    fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        self.block().replace_child(old, new.id)
    }
    fn update(&mut self, _frame: &eframe::Frame) {}
    fn finish_frame(&mut self) {}
    fn set_tab_active(&mut self, _active: bool) {}
    fn tab_closed(&mut self) {}

    fn wants_presence(&self) -> bool {
        true
    }

    fn sync_cursor_presence(&mut self, _client: &BlockClient, _visible: bool) {}

    fn reveal_presence_cursor(&mut self, _client_id: block::ClientId) {}
    fn history(&self) -> Option<&dyn BlockHistoryHandle> {
        self.block().history()
    }
    fn render(
        &mut self,
        _context: BlockRenderContext<'_>,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        false
    }
    fn render_aspect_ratio(&self) -> Option<f32> {
        None
    }
    fn default_preserve_aspect_ratio(&self) -> bool {
        false
    }
    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities;
    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        DirectEditorInteraction::Preview
    }
    fn direct_editor_resize(&self) -> DirectEditorResize {
        DirectEditorResize::None
    }
    fn direct_editor_fills_viewport(&self) -> bool {
        false
    }

    fn direct_editor_max_zoom(&self) -> f32 {
        DIRECT_EDITOR_MAX_ZOOM
    }
    fn direct_editor_min_zoom(&self) -> f32 {
        DIRECT_EDITOR_MIN_ZOOM
    }
    fn direct_editor_viewport_input(
        &self,
        _editors: &EditorAccess<'_>,
    ) -> DirectEditorViewportInput {
        DirectEditorViewportInput::Background
    }
    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        None
    }
    fn direct_editor_intrinsic_size_for_width(
        &mut self,
        width: f32,
        editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let mut size = self.direct_editor_intrinsic_size(editors)?;
        size.x = width;
        Some(size)
    }
    fn set_direct_editor_intrinsic_size(
        &mut self,
        _size: egui::Vec2,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        false
    }
    fn direct_editor_top_bar(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        false
    }
    fn direct_editor_left_sidebar(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        false
    }
    fn direct_editor_right_sidebar(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_ui(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        None
    }
    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.direct_editor_ui(ui, editors, scale, viewport)
    }
}

pub fn direct_editor_tab_ui(
    editor: &mut dyn BlockEditor,
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
) -> Option<EditorAction> {
    let id = editor.id();
    let compact = ui.available_width() < COMPACT_DIRECT_EDITOR_WIDTH;
    let read_only = !editors.access().can_edit();
    let mut action = None;
    let capabilities = editor.direct_editor_capabilities();
    let max_zoom = editor.direct_editor_max_zoom();
    let min_zoom = editor.direct_editor_min_zoom();
    let viewport_id = egui::Id::new(("direct-editor-tab-viewport", id));
    let mut viewport_state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<DirectEditorTabViewport>(viewport_id))
        .unwrap_or_default();
    let mut viewport = DirectEditorViewport::new(viewport_state.zoom);

    egui::Panel::top(egui::Id::new(("direct-editor-tab-toolbar", id)))
        .show_separator_line(true)
        .show_inside(ui, |ui| {
            let next_action = editor_scope(ui, read_only, |ui| {
                editor.direct_editor_top_bar(ui, editors, &mut viewport)
            });
            if action.is_none() {
                action = next_action;
            }
        });

    if compact {
        let available = ui.available_rect_before_wrap();
        if editor.direct_editor_has_left_sidebar(editors) {
            egui::Window::new("Left sidebar")
                .id(egui::Id::new(("direct-editor-tab-left-window", id)))
                .default_width(240.0)
                .resizable(true)
                .default_pos(available.left_top() + egui::vec2(16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let next_action = editor_scope(ui, read_only, |ui| {
                                editor.direct_editor_left_sidebar(ui, editors)
                            });
                            if action.is_none() {
                                action = next_action;
                            }
                        });
                });
        }
        if editor.direct_editor_has_right_sidebar(editors) {
            egui::Window::new("Right sidebar")
                .id(egui::Id::new(("direct-editor-tab-right-window", id)))
                .pivot(egui::Align2::RIGHT_TOP)
                .default_width(240.0)
                .resizable(true)
                .default_pos(available.right_top() + egui::vec2(-16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let next_action = editor_scope(ui, read_only, |ui| {
                                editor.direct_editor_right_sidebar(ui, editors)
                            });
                            if action.is_none() {
                                action = next_action;
                            }
                        });
                });
        }
    } else {
        if editor.direct_editor_has_left_sidebar(editors) {
            egui::Panel::left(egui::Id::new(("direct-editor-tab-left", id)))
                .default_size(240.0)
                .min_size(200.0)
                .max_size(340.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let next_action = editor_scope(ui, read_only, |ui| {
                        editor.direct_editor_left_sidebar(ui, editors)
                    });
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
        if editor.direct_editor_has_right_sidebar(editors) {
            egui::Panel::right(egui::Id::new(("direct-editor-tab-right", id)))
                .default_size(240.0)
                .min_size(200.0)
                .max_size(340.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let next_action = editor_scope(ui, read_only, |ui| {
                        editor.direct_editor_right_sidebar(ui, editors)
                    });
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
    }

    let viewport_size = ui.available_size().max(egui::Vec2::splat(1.0));
    let intrinsic_size = editor
        .direct_editor_intrinsic_size(editors)
        .unwrap_or_default();
    let content_size = egui::vec2(
        viewport_size.x.max(intrinsic_size.x),
        viewport_size.y.max(intrinsic_size.y),
    );
    if capabilities.supports_pan_and_zoom {
        let (viewport_rect, _) = ui.allocate_exact_size(viewport_size, egui::Sense::hover());
        if let Some(previous_center) = viewport_state.center.replace(viewport_rect.center()) {
            viewport_state.pan += previous_center - viewport_rect.center();
        }
        let transformed_size = content_size * viewport_state.zoom;
        let content_rect = egui::Rect::from_center_size(
            viewport_rect.center() + viewport_state.pan,
            transformed_size,
        );
        viewport.replace_content_rect(Some(content_rect));
        let fills_viewport = editor.direct_editor_fills_viewport();

        let mut viewport_input = editor.direct_editor_viewport_input(editors);
        if read_only && viewport_input == DirectEditorViewportInput::Editor {
            viewport_input = DirectEditorViewportInput::Background;
        }
        let editor_rect = if fills_viewport {
            viewport_rect
        } else {
            content_rect
        };
        let next_action = ui
            .new_child(
                egui::UiBuilder::new()
                    .id_salt(("direct-editor-tab-content", id))
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .scope(|ui| {
                ui.set_clip_rect(viewport_rect.intersect(ui.clip_rect()));
                ui.set_min_size(editor_rect.size());
                editor_scope(ui, read_only, |ui| {
                    editor.direct_editor_ui(ui, editors, viewport_state.zoom, &mut viewport)
                })
            })
            .inner;
        if action.is_none() {
            action = next_action;
        }

        match viewport_input {
            DirectEditorViewportInput::Editor => {}
            DirectEditorViewportInput::Background => viewport_gesture_input(
                ui.ctx(),
                viewport_rect,
                (!read_only).then_some(content_rect),
                &mut viewport,
            ),
            DirectEditorViewportInput::Viewport => {
                viewport_gesture_input(ui.ctx(), viewport_rect, None, &mut viewport)
            }
        }

        for command in viewport.drain() {
            match command {
                DirectEditorViewportCommand::Pan(delta) => {
                    viewport_state.pan += delta;
                    if let Some(auto_fit) = &mut viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Zoom { factor, anchor } => {
                    let old_zoom = viewport_state.zoom;
                    let new_zoom = (old_zoom * factor).clamp(min_zoom, max_zoom);
                    if new_zoom != old_zoom {
                        let anchor = anchor.unwrap_or_else(|| viewport_rect.center());
                        viewport_state.pan = (anchor - viewport_rect.center())
                            - ((anchor - viewport_rect.center()) - viewport_state.pan)
                                * (new_zoom / old_zoom);
                        viewport_state.zoom = new_zoom;
                    }
                    if let Some(auto_fit) = &mut viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Fit => {
                    fit_direct_editor_viewport(
                        &mut viewport_state,
                        viewport_size,
                        content_size,
                        min_zoom,
                    );
                    if let Some(auto_fit) = &mut viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::AutoFit(target) => {
                    let auto_fit = viewport_state.auto_fit.get_or_insert(AutoFitState {
                        target,
                        enabled: true,
                    });
                    if auto_fit.target != target {
                        *auto_fit = AutoFitState {
                            target,
                            enabled: true,
                        };
                    }
                    if auto_fit.enabled {
                        fit_direct_editor_viewport(
                            &mut viewport_state,
                            viewport_size,
                            content_size,
                            min_zoom,
                        );
                    }
                }
                DirectEditorViewportCommand::ResumeAutoFit => {
                    if let Some(auto_fit) = &mut viewport_state.auto_fit {
                        auto_fit.enabled = true;
                        fit_direct_editor_viewport(
                            &mut viewport_state,
                            viewport_size,
                            content_size,
                            min_zoom,
                        );
                    }
                }
            }
        }
        ui.ctx()
            .data_mut(|data| data.insert_temp(viewport_id, viewport_state));
    } else {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_size(content_size);
                let next_action = editor_scope(ui, read_only, |ui| {
                    editor.direct_editor_ui(ui, editors, 1.0, &mut viewport)
                });
                if action.is_none() {
                    action = next_action;
                }
            });
    }

    action
}

fn viewport_gesture_input(
    context: &egui::Context,
    viewport_rect: egui::Rect,
    steered: Option<egui::Rect>,
    viewport: &mut DirectEditorViewport,
) {
    let Some(pointer) = context.pointer_hover_pos().filter(|pointer| {
        viewport_rect.contains(*pointer) && !steered.is_some_and(|rect| rect.contains(*pointer))
    }) else {
        return;
    };
    let (scroll, zoom_delta, command, panning, delta) = context.input(|input| {
        (
            input.smooth_scroll_delta,
            input.zoom_delta(),
            input.modifiers.command,
            input.pointer.button_down(egui::PointerButton::Middle)
                || (input.key_down(egui::Key::Space)
                    && input.pointer.button_down(egui::PointerButton::Primary)),
            input.pointer.delta(),
        )
    });
    if panning {
        context.set_cursor_icon(egui::CursorIcon::Grabbing);
        viewport.pan(delta);
    }
    if (zoom_delta - 1.0).abs() > f32::EPSILON {
        viewport.change_zoom(zoom_delta, Some(pointer));
    } else if command && scroll.y != 0.0 {
        viewport.change_zoom((scroll.y * 0.002).exp(), Some(pointer));
    } else if scroll != egui::Vec2::ZERO {
        viewport.pan(scroll);
    }
}

fn fit_direct_editor_viewport(
    viewport: &mut DirectEditorTabViewport,
    viewport_size: egui::Vec2,
    content_size: egui::Vec2,
    min_zoom: f32,
) {
    viewport.zoom = (viewport_size.x / content_size.x)
        .min(viewport_size.y / content_size.y)
        .min(1.0)
        .clamp(min_zoom, DIRECT_EDITOR_MAX_ZOOM);
    viewport.pan = egui::Vec2::ZERO;
}

#[derive(Clone, Copy, Debug)]
struct AutoFitState {
    target: Uuid,
    enabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct DirectEditorTabViewport {
    zoom: f32,
    pan: egui::Vec2,
    center: Option<egui::Pos2>,
    auto_fit: Option<AutoFitState>,
}

impl Default for DirectEditorTabViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            center: None,
            auto_fit: None,
        }
    }
}

type CreateEditor = Box<dyn Fn(&BlockClient) -> Box<dyn BlockEditor>>;
type OpenEditor = Box<dyn Fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>>;
type CreateOptions = Box<dyn Fn() -> Box<dyn PendingCreation>>;
type RegenerateDynamicArtifact =
    fn(&BlockClient, Uuid, Uuid, &[u8]) -> Result<Box<dyn DynamicArtifactRegeneration>, String>;

pub trait DynamicArtifactRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>>;
}

#[derive(Clone, Copy)]
pub(super) struct DynamicArtifactSupport {
    pub source: fn(&[u8]) -> Result<Uuid, String>,

    pub summary: fn(&[u8]) -> String,

    pub settings_ui: fn(&mut egui::Ui, &mut Vec<u8>) -> bool,
    pub regenerate: RegenerateDynamicArtifact,
}

enum ArtifactProvider {
    Native(DynamicArtifactSupport),
    Plugin(Arc<PluginManifest>),
}

pub(super) trait ArtifactSession {
    fn poll(
        &mut self,
        ctx: &egui::Context,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        data: &[u8],
    ) -> ArtifactStatus;
    fn settings_ui(
        &mut self,
        ui: &mut egui::Ui,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        draft: &mut Vec<u8>,
    );
    fn summary(&self, draft: &[u8]) -> Option<String>;
    fn cancel_settings(&mut self);
    fn regenerate(&mut self, client: &Arc<BlockClient>, data: &[u8]);
    fn take_outcome(&mut self) -> Option<Result<(), String>>;
    fn regenerating(&self) -> bool;
}

pub(super) enum ArtifactStatus {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}

struct NativeArtifactSession {
    support: DynamicArtifactSupport,
    target_id: Uuid,
    target_type: Uuid,
    regeneration: Option<Box<dyn DynamicArtifactRegeneration>>,
    outcome: Option<Result<(), String>>,
}

impl ArtifactSession for NativeArtifactSession {
    fn poll(
        &mut self,
        _ctx: &egui::Context,
        _registry: &EditorRegistry,
        _client: &Arc<BlockClient>,
        data: &[u8],
    ) -> ArtifactStatus {
        if let Some(regeneration) = &mut self.regeneration {
            if let Some(result) = regeneration.poll() {
                self.regeneration = None;
                self.outcome = Some(result);
            }
        }
        match (self.support.source)(data) {
            Ok(source) => ArtifactStatus::Described {
                source,
                summary: (self.support.summary)(data),
            },
            Err(error) => ArtifactStatus::Failed(error),
        }
    }

    fn settings_ui(
        &mut self,
        ui: &mut egui::Ui,
        _registry: &EditorRegistry,
        _client: &Arc<BlockClient>,
        draft: &mut Vec<u8>,
    ) {
        (self.support.settings_ui)(ui, draft);
    }

    fn summary(&self, draft: &[u8]) -> Option<String> {
        Some((self.support.summary)(draft))
    }

    fn cancel_settings(&mut self) {}

    fn regenerate(&mut self, client: &Arc<BlockClient>, data: &[u8]) {
        self.outcome = None;
        match (self.support.regenerate)(client, self.target_id, self.target_type, data) {
            Ok(regeneration) => self.regeneration = Some(regeneration),
            Err(error) => self.outcome = Some(Err(error)),
        }
    }

    fn take_outcome(&mut self) -> Option<Result<(), String>> {
        self.outcome.take()
    }

    fn regenerating(&self) -> bool {
        self.regeneration.is_some()
    }
}

pub(super) trait EditorKind: BlockEditor + Sized + 'static {
    type Block: Block;

    const DISPLAY_NAME: &'static str;
    const ICON: MaterialIcon;

    const CAN_ADD_CHILD: bool = false;
    const CAN_DELETE_CHILD: bool = false;
    const CAN_REPLACE_CHILD: bool = false;

    const DEFAULT_IMPORTANT: bool = false;

    fn open(client: &BlockClient, block: BlockHandle<Self::Block>) -> Self;

    fn dynamic_artifact() -> Option<DynamicArtifactSupport> {
        None
    }
}

pub(super) trait CreatableEditor: EditorKind {
    fn create(client: &BlockClient) -> Self;
}

pub(super) trait ConfigurableEditor: EditorKind {
    type Options: CreationOptions;

    fn create(client: &BlockClient, options: Self::Options) -> Result<Self, String>;
}

pub(super) trait CreationOptions: Default {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool;
}

pub(super) trait PendingCreation {
    fn ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> CreationStep;
    fn create(&mut self, client: &BlockClient) -> Result<Option<Box<dyn BlockEditor>>, String>;
}

pub(super) enum CreationStep {
    Options(bool),
    Working,
}

struct EditorCreation<E: ConfigurableEditor> {
    options: E::Options,
}

impl<E: ConfigurableEditor> PendingCreation for EditorCreation<E> {
    fn ui(&mut self, ui: &mut egui::Ui, _editors: &mut EditorAccess<'_>) -> CreationStep {
        CreationStep::Options(self.options.ui(ui))
    }

    fn create(&mut self, client: &BlockClient) -> Result<Option<Box<dyn BlockEditor>>, String> {
        E::create(client, std::mem::take(&mut self.options))
            .map(|editor| Some(Box::new(editor) as Box<dyn BlockEditor>))
    }
}

pub(super) fn image_filter() -> FileFilter {
    FileFilter::new("Images", "Image", Image::FILE_EXTENSIONS, Image::MIME_TYPES)
}

pub(super) fn imported_image(file: PickedFile) -> Image {
    let PickedFile { name, data } = file;
    Image::new(name, data)
}

pub(super) fn create_image_block(
    editors: &mut EditorAccess<'_>,
    image: Image,
    parent: Uuid,
) -> Uuid {
    let block = editors.client().create_block(image);
    let id = block.id();
    block.set_parent(BlockParent::Uuid(parent));
    let editor = editors
        .registry()
        .open(editors.client(), id, Image::TYPE_ID);
    editors.insert(editor);
    id
}

pub(super) enum BlockCreation {
    Created(Box<dyn BlockEditor>),
    Options(Box<dyn PendingCreation>),
}

enum CreateBlock {
    Immediate(CreateEditor),

    Configured(CreateOptions),
}

struct EditorRegistration {
    block_type: Uuid,
    display_name: &'static str,
    icon: MaterialIcon,
    create: Option<CreateBlock>,
    open: OpenEditor,
    can_add_child: bool,
    can_delete_child: bool,
    can_replace_child: bool,
    default_important: bool,
    dynamic_artifact: Option<ArtifactProvider>,
}

impl EditorRegistration {
    fn of<E: EditorKind>() -> Self {
        Self {
            block_type: E::Block::TYPE_ID,
            display_name: E::DISPLAY_NAME,
            icon: E::ICON,
            create: None,
            open: Box::new(|client, id| {
                Box::new(E::open(client, client.get_block::<E::Block>(id)))
            }),
            can_add_child: E::CAN_ADD_CHILD,
            can_delete_child: E::CAN_DELETE_CHILD,
            can_replace_child: E::CAN_REPLACE_CHILD,
            default_important: E::DEFAULT_IMPORTANT,
            dynamic_artifact: E::dynamic_artifact().map(ArtifactProvider::Native),
        }
    }
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    new_block_actions: Vec<(&'static str, Uuid, bool)>,
    plugin_block_types: Arc<Vec<block_plugin_api::BlockTypeDescriptor>>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            new_block_actions: Vec::new(),
            plugin_block_types: Arc::default(),
        };
        registry.register_configurable::<deterministic_game::DeterministicGameEditor>();
        registry.register_creatable::<infinite_canvas::InfiniteCanvasEditor>();
        registry.register_creatable::<logic_grid::LogicGridEditor>();
        registry.register_creatable::<map::MapEditor>();
        registry.register_creatable::<pixel_ray_tracer::PixelRayTracerEditor>();
        registry.register_creatable::<scene_3d::Scene3DEditor>();
        registry.register_creatable::<text::TextEditor>();
        registry.register::<version_control_worktree::VersionControlWorktreeEditor>();
        registry.register_creatable::<video::VideoEditor>();
        registry.register_creatable::<browser_tab::WebBrowserTabEditor>();
        for manifest in plugin::discovery::manifests() {
            registry.register_plugin(manifest);
        }
        registry.plugin_block_types =
            Arc::new(plugin::block_type_descriptors(registry.block_types()));
        registry
    }

    fn block_types(&self) -> Vec<(Uuid, BlockTypeEntry)> {
        let mut types: Vec<_> = self
            .registrations
            .values()
            .map(|registration| {
                (
                    registration.block_type,
                    BlockTypeEntry {
                        display_name: registration.display_name.to_owned(),
                        icon: Some(registration.icon),
                    },
                )
            })
            .collect();
        types.sort_by_key(|(block_type, _)| *block_type);
        types
    }

    pub(super) fn plugin_block_types(&self) -> &Arc<Vec<block_plugin_api::BlockTypeDescriptor>> {
        &self.plugin_block_types
    }

    fn register<E: EditorKind>(&mut self) {
        self.insert(EditorRegistration::of::<E>());
    }

    fn register_creatable<E: CreatableEditor>(&mut self) {
        let mut registration = EditorRegistration::of::<E>();
        registration.create = Some(CreateBlock::Immediate(Box::new(|client| {
            Box::new(E::create(client))
        })));
        self.insert(registration);
    }

    fn register_configurable<E: ConfigurableEditor>(&mut self) {
        let mut registration = EditorRegistration::of::<E>();
        registration.create = Some(CreateBlock::Configured(Box::new(|| {
            Box::new(EditorCreation::<E> {
                options: E::Options::default(),
            })
        })));
        self.insert(registration);
    }

    fn insert(&mut self, registration: EditorRegistration) {
        if registration.create.is_some() {
            self.new_block_actions.push((
                registration.display_name,
                registration.block_type,
                registration.default_important,
            ));
        }
        self.registrations
            .insert(registration.block_type, registration);
    }

    fn register_plugin(&mut self, manifest: Arc<PluginManifest>) {
        let block_type = Uuid::from_bytes(manifest.block_type);
        let display_name: &'static str = Box::leak(manifest.display_name.clone().into_boxed_str());
        let icon = MaterialIcon::new(Box::leak(manifest.icon.clone().into_boxed_str()));
        self.insert(EditorRegistration {
            block_type,
            display_name,
            icon,
            create: match manifest.creation {
                block_plugin_api::CreationMode::Immediate
                | block_plugin_api::CreationMode::Dialog => {
                    let manifest = Arc::clone(&manifest);
                    Some(CreateBlock::Configured(Box::new(move || {
                        Box::new(plugin::PluginCreation::new(Arc::clone(&manifest)))
                    })))
                }
                block_plugin_api::CreationMode::None => None,
            },
            open: {
                let manifest = Arc::clone(&manifest);
                Box::new(move |client, id| {
                    let block = blocks::open(client, id, block_type)
                        .expect("a registered plugin block type is in the erased table");
                    Box::new(plugin::PluginEditor::new(Arc::clone(&manifest), block))
                })
            },
            can_add_child: manifest.children.add,
            can_delete_child: manifest.children.delete,
            can_replace_child: manifest.children.replace,
            default_important: manifest.important,
            dynamic_artifact: manifest
                .regions
                .contains(&block_plugin_api::EditorRegion::ArtifactSettings)
                .then(|| ArtifactProvider::Plugin(Arc::clone(&manifest))),
        });
    }

    pub fn new_block_actions(&self) -> &[(&'static str, Uuid, bool)] {
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

    pub fn can_replace_child(&self, block_type: Uuid) -> bool {
        self.registrations
            .get(&block_type)
            .is_some_and(|registration| registration.can_replace_child)
    }

    pub(super) fn artifact_session(
        &self,
        source_type: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Result<Box<dyn ArtifactSession>, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        match &registration.dynamic_artifact {
            Some(ArtifactProvider::Native(support)) => Ok(Box::new(NativeArtifactSession {
                support: *support,
                target_id,
                target_type,
                regeneration: None,
                outcome: None,
            })),
            Some(ArtifactProvider::Plugin(manifest)) => Ok(Box::new(plugin::PluginArtifact::new(
                Arc::clone(manifest),
                target_id,
                target_type,
                client_id,
            ))),
            None => Err(format!(
                "{} blocks do not generate dynamic artifacts",
                registration.display_name
            )),
        }
    }

    pub(super) fn create(&self, client: &BlockClient, block_type: Uuid) -> Option<BlockCreation> {
        match self.registrations.get(&block_type)?.create.as_ref()? {
            CreateBlock::Immediate(create) => Some(BlockCreation::Created(create(client))),
            CreateBlock::Configured(options) => Some(BlockCreation::Options(options())),
        }
    }

    pub fn open(&self, client: &BlockClient, id: Uuid, block_type: Uuid) -> Box<dyn BlockEditor> {
        self.registrations.get(&block_type).map_or_else(
            || Box::new(UnsupportedEditor::new(id, block_type)) as Box<dyn BlockEditor>,
            |registration| (registration.open)(client, id),
        )
    }
}

impl BlockTypes for EditorRegistry {
    fn display_name(&self, block_type: Uuid) -> Option<&str> {
        Self::display_name(self, block_type)
    }

    fn icon(&self, block_type: Uuid) -> Option<MaterialIcon> {
        Self::icon(self, block_type)
    }
}
