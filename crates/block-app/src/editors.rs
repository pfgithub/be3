mod audio;
// The embedded browser is a native webview. Android and the browser sandbox
// have no equivalent, so those builds fall back to UnsupportedEditor.
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod browser_tab;
mod calendar;
mod clipboard;
mod compiled_logic;
mod database;
mod database_schema;
mod database_view;
mod gui_builder;
mod hotbar;
pub(crate) mod image;
pub(crate) mod infinite_canvas;
mod logic_game;
mod logic_grid;
mod map;
mod pixel_art;
mod pixel_ray_tracer;
#[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "android"))]
mod plugin;
mod presentation;
mod reference_cache;
mod scene_3d;
pub(crate) mod settings;
mod text;
mod ui_settings;
mod unsupported;
mod version_control_data;
mod version_control_worktree;
mod video;
mod workspace_index;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use block::{Block, BlockAccess, BlockParent, BlockReference};
use block_client::{
    blocks::workspace_index::BlockEntry, BlockClient, BlockHandle, BlockHandleAccess,
    BlockHistoryHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_LOCK, MaterialIcon};
use uuid::Uuid;

use self::unsupported::UnsupportedEditor;

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

/// Sets up the render resources editors need before any of them draws. eframe
/// hands these out once, at startup, so they are claimed here rather than from
/// inside an editor.
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

/// The most an editor may ever do with a block: what the server grants, and
/// never more than viewing for a generated artifact, whose contents are
/// replaced wholesale the next time it is regenerated.
pub fn editor_access_ceiling(client: &BlockClient, id: Uuid) -> BlockAccess {
    let access = client.block_access(id);
    if client.is_dynamic_artifact(id) {
        access.min(BlockAccess::View)
    } else {
        access
    }
}

/// Draws `contents`, non-interactive when the block may only be viewed. The
/// fade a disabled `Ui` normally gets is turned off: a block being read should
/// stay as legible as one being edited.
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

/// Replaces an editor the account may only know the existence of.
fn no_access_notice(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.weak(format!("{} No access", ICON_LOCK.codepoint));
    });
}

/// An unrotated rectangle as the corners `BlockRenderContext` wants.
fn rect_corners(rect: egui::Rect) -> [egui::Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

/// The largest centered rectangle of `ratio` that fits inside `available`.
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

/// A block's icon and display name, along with whether the name was
/// auto-derived from the block's content rather than chosen by the user.
/// Shared by every place in the app that shows a block's name, so an
/// automatic name can be marked as such (e.g. italicized) consistently.
pub(super) struct BlockLabel {
    pub block_type: Uuid,
    pub icon: Option<MaterialIcon>,
    pub name: String,
    pub automatic: bool,
}

impl BlockLabel {
    fn new(
        registry: &EditorRegistry,
        block_type: Uuid,
        name: Option<&block_client::properties::BlockName>,
    ) -> Self {
        let (name, automatic) = match name.filter(|name| !name.value.is_empty()) {
            Some(name) => (name.value.clone(), !name.manual),
            None => (
                registry
                    .display_name(block_type)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "Untitled".to_owned()),
                true,
            ),
        };
        Self {
            block_type,
            icon: registry.icon(block_type),
            name,
            automatic,
        }
    }

    /// For a block type and its raw property map, e.g. from a
    /// [`BlockReference`] or [`block_client::CachedBlock`].
    pub(super) fn for_properties(
        registry: &EditorRegistry,
        block_type: Uuid,
        properties: &std::collections::BTreeMap<Uuid, Vec<u8>>,
    ) -> Self {
        Self::new(
            registry,
            block_type,
            block_client::properties::read_name(properties).as_ref(),
        )
    }

    /// For a listed [`BlockReference`].
    pub(super) fn for_reference(registry: &EditorRegistry, reference: &BlockReference) -> Self {
        Self::for_properties(registry, reference.block_type, &reference.properties)
    }

    /// For a [`block_client::CachedBlock`].
    pub(super) fn for_cached(
        registry: &EditorRegistry,
        cached: &block_client::CachedBlock,
    ) -> Self {
        Self::for_properties(registry, cached.block_type, &cached.properties)
    }

    /// For a block whose editor is open locally.
    pub(super) fn for_handle(registry: &EditorRegistry, handle: &dyn BlockHandleAccess) -> Self {
        Self::new(registry, handle.block_type(), handle.block_name().as_ref())
    }

    /// The name alone, italicized if it was auto-derived rather than chosen
    /// by the user.
    pub(super) fn rich_text(&self) -> egui::RichText {
        let text = egui::RichText::new(&self.name);
        if self.automatic {
            text.italics()
        } else {
            text
        }
    }

    /// Icon and name combined for a widget (button, label, ...), the name
    /// italicized if automatic.
    pub(super) fn widget_text(&self, style: &egui::Style) -> egui::WidgetText {
        let Some(icon) = self.icon else {
            return self.rich_text().into();
        };
        let mut job = egui::text::LayoutJob::default();
        egui::RichText::new(format!("{} ", icon.codepoint)).append_to(
            &mut job,
            style,
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        self.rich_text().append_to(
            &mut job,
            style,
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        job.into()
    }
}

/// Lays out `text` for direct painting, matching
/// [`egui::Painter::layout_no_wrap`] but italicizing it when `automatic` -
/// for marking an auto-derived block name in painter-based (non-widget)
/// rendering.
pub(super) fn name_galley(
    painter: &egui::Painter,
    text: &str,
    font_id: egui::FontId,
    color: egui::Color32,
    automatic: bool,
) -> std::sync::Arc<egui::Galley> {
    if !automatic {
        return painter.layout_no_wrap(text.to_owned(), font_id, color);
    }
    painter.layout_job(egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::text::TextFormat {
            font_id,
            color,
            italics: true,
            ..Default::default()
        },
    ))
}

/// [`egui::Painter::text`], but italicizing the text when `automatic`.
pub(super) fn paint_name(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: &str,
    font_id: egui::FontId,
    color: egui::Color32,
    automatic: bool,
) -> egui::Rect {
    let galley = name_galley(painter, text, font_id, color, automatic);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

/// Stands in for a block whose preview could not be drawn, naming the block
/// and showing the icon of its type.
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
    ui.new_child(
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
    .inner
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

    /// What the editor currently being drawn may do with its block.
    pub fn access(&self) -> BlockAccess {
        self.access
    }

    /// What a block nested in the current editor may be shown as. An editor
    /// never lets a block inside it be changed more than it is itself.
    fn access_for(&self, id: Uuid) -> BlockAccess {
        self.access.min(editor_access_ceiling(self.client, id))
    }

    pub fn client(&self) -> &BlockClient {
        self.client
    }

    pub fn client_handle(&self) -> Arc<BlockClient> {
        Arc::clone(self.client)
    }

    /// This installation's identity, used to pick out per-client settings
    /// entries.
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

    /// Draws a nested editor at the access it is allowed: blocks that may only
    /// be known to exist are replaced by a notice, and blocks that may only be
    /// viewed are drawn without any way to change them.
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

    pub fn direct_editor_handles_viewport_input(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.direct_editor_handles_viewport_input(self))
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

    pub fn direct_editor_ui(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.direct_editor_ui(ui, editors, scale, viewport)
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
    /// Whether `UserActive` presence may be posted for this editor's block
    /// while its tab is visible. Presence requires the block to be watched
    /// on the server, which every real editor establishes by calling
    /// `get_block` when it opens; the unsupported fallback never does, so it
    /// overrides this to `false` to avoid a `NotWatching` error.
    fn wants_presence(&self) -> bool {
        true
    }
    /// Posts (or clears) presence describing where this editor's cursor or
    /// selection currently is, if it has one. Called every frame with the
    /// same on-screen signal that drives `UserActive` presence, so a cursor
    /// is announced for exactly as long as its block is visible.
    fn sync_cursor_presence(&mut self, _client: &BlockClient, _visible: bool) {}
    /// Requests that the editor scroll its view to the given client's cursor
    /// or selection, if it tracks one, the next time it draws.
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
    /// Upper limit for the tab viewport zoom; editors with deep content can
    /// raise it above the shared default.
    fn direct_editor_max_zoom(&self) -> f32 {
        DIRECT_EDITOR_MAX_ZOOM
    }
    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        false
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
        // A read-only editor answers no input, so the tab drives the viewport
        // for it. Editors read the content rect back, so panning and zooming
        // still reach the ones that normally steer it themselves.
        let handles_viewport_input =
            !read_only && editor.direct_editor_handles_viewport_input(editors);
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

        if !handles_viewport_input
            && handle_direct_editor_background_input(
                ui.ctx(),
                viewport_rect,
                (!read_only).then_some(content_rect),
                &mut viewport_state,
                max_zoom,
            )
        {
            if let Some(auto_fit) = &mut viewport_state.auto_fit {
                auto_fit.enabled = false;
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
                    let new_zoom = (old_zoom * factor).clamp(DIRECT_EDITOR_MIN_ZOOM, max_zoom);
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
                    fit_direct_editor_viewport(&mut viewport_state, viewport_size, content_size);
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

/// Pans and zooms the tab viewport. `content_rect` is the area the editor
/// steers itself, which is left out; a read-only editor steers nothing and
/// passes `None`.
fn handle_direct_editor_background_input(
    context: &egui::Context,
    viewport_rect: egui::Rect,
    content_rect: Option<egui::Rect>,
    viewport: &mut DirectEditorTabViewport,
    max_zoom: f32,
) -> bool {
    let Some(pointer) = context.pointer_hover_pos().filter(|pointer| {
        viewport_rect.contains(*pointer)
            && !content_rect.is_some_and(|content| content.contains(*pointer))
    }) else {
        return false;
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
        viewport.pan += delta;
    }
    let zoom_factor = if (zoom_delta - 1.0).abs() > f32::EPSILON {
        Some(zoom_delta)
    } else if command && scroll.y != 0.0 {
        Some((scroll.y * 0.002).exp())
    } else {
        None
    };
    if let Some(zoom_factor) = zoom_factor {
        let old_zoom = viewport.zoom;
        let new_zoom = (old_zoom * zoom_factor).clamp(DIRECT_EDITOR_MIN_ZOOM, max_zoom);
        if new_zoom != old_zoom {
            viewport.pan = (pointer - viewport_rect.center())
                - ((pointer - viewport_rect.center()) - viewport.pan) * (new_zoom / old_zoom);
            viewport.zoom = new_zoom;
        }
    } else if scroll != egui::Vec2::ZERO {
        viewport.pan += scroll;
    }
    panning || zoom_factor.is_some() || scroll != egui::Vec2::ZERO
}

fn fit_direct_editor_viewport(
    viewport: &mut DirectEditorTabViewport,
    viewport_size: egui::Vec2,
    content_size: egui::Vec2,
) {
    viewport.zoom = (viewport_size.x / content_size.x)
        .min(viewport_size.y / content_size.y)
        .min(1.0)
        .clamp(DIRECT_EDITOR_MIN_ZOOM, DIRECT_EDITOR_MAX_ZOOM);
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

type CreateEditor = fn(&BlockClient) -> Box<dyn BlockEditor>;
type OpenEditor = fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>;
type RegenerateDynamicArtifact =
    fn(&BlockClient, Uuid, Uuid, &[u8]) -> Result<Box<dyn DynamicArtifactRegeneration>, String>;

pub trait DynamicArtifactRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>>;
}

/// What a source block type can tell the app about the artifacts it produces.
/// The descriptor payload is opaque to everything but these functions.
#[derive(Clone, Copy)]
pub(super) struct DynamicArtifactSupport {
    /// The block the artifact was generated from.
    pub source: fn(&[u8]) -> Result<Uuid, String>,
    /// A short description of what the current settings produce.
    pub summary: fn(&[u8]) -> String,
    /// Edits the payload in place; `true` when the settings changed.
    pub settings_ui: fn(&mut egui::Ui, &mut Vec<u8>) -> bool,
    pub regenerate: RegenerateDynamicArtifact,
}

/// How an editor is registered. Only the block type, name, icon and `open`
/// are required: every other item describes an optional capability and
/// defaults to not having it.
pub(super) trait EditorKind: BlockEditor + Sized + 'static {
    /// The block type this editor edits. Its `TYPE_ID` identifies the editor.
    type Block: Block;

    const DISPLAY_NAME: &'static str;
    const ICON: MaterialIcon;
    /// Set these only alongside the matching `BlockEditor` method.
    const CAN_ADD_CHILD: bool = false;
    const CAN_DELETE_CHILD: bool = false;
    const CAN_REPLACE_CHILD: bool = false;
    /// Whether this editor is common enough to show in the main section of
    /// the add-block picker, rather than below it.
    const DEFAULT_IMPORTANT: bool = false;

    fn open(client: &BlockClient, block: BlockHandle<Self::Block>) -> Self;

    /// What this block type can say about the artifacts it generates.
    fn dynamic_artifact() -> Option<DynamicArtifactSupport> {
        None
    }
}

/// Editors whose block is created on the spot. Types that need something from
/// the user first implement `ConfigurableEditor` instead.
pub(super) trait CreatableEditor: EditorKind {
    fn create(client: &BlockClient) -> Self;
}

/// Editors whose block cannot be created until the user supplies something,
/// such as the file behind an image. The options are collected in a dialog
/// and handed to `create` when it is accepted.
pub(super) trait ConfigurableEditor: EditorKind {
    type Options: CreationOptions;

    fn create(client: &BlockClient, options: Self::Options) -> Result<Self, String>;
}

/// The options one block type needs before it can be created. The dialog
/// around them - its frame, its buttons and its errors - is shared, so this
/// draws only the options themselves.
pub(super) trait CreationOptions: Default {
    /// Draws the options. Returns `false` while they are incomplete, which
    /// keeps the create button disabled.
    fn ui(&mut self, ui: &mut egui::Ui) -> bool;
}

/// A creation dialog waiting on the user: the options being filled in, and
/// how to turn them into an editor.
pub(super) trait PendingCreation {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool;
    fn create(&mut self, client: &BlockClient) -> Result<Box<dyn BlockEditor>, String>;
}

struct EditorCreation<E: ConfigurableEditor> {
    options: E::Options,
}

impl<E: ConfigurableEditor> PendingCreation for EditorCreation<E> {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        self.options.ui(ui)
    }

    fn create(&mut self, client: &BlockClient) -> Result<Box<dyn BlockEditor>, String> {
        E::create(client, std::mem::take(&mut self.options))
            .map(|editor| Box::new(editor) as Box<dyn BlockEditor>)
    }
}

/// Starting to create a block either produces the editor outright or the
/// dialog that has to be filled in first.
pub(super) enum BlockCreation {
    Created(Box<dyn BlockEditor>),
    Options(Box<dyn PendingCreation>),
}

enum CreateBlock {
    /// Nothing to ask about: the block is created on the spot.
    Immediate(CreateEditor),
    /// Options are collected before the block exists.
    Configured(fn() -> Box<dyn PendingCreation>),
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
    dynamic_artifact: Option<DynamicArtifactSupport>,
}

impl EditorRegistration {
    fn of<E: EditorKind>() -> Self {
        Self {
            block_type: E::Block::TYPE_ID,
            display_name: E::DISPLAY_NAME,
            icon: E::ICON,
            create: None,
            open: |client, id| Box::new(E::open(client, client.get_block::<E::Block>(id))),
            can_add_child: E::CAN_ADD_CHILD,
            can_delete_child: E::CAN_DELETE_CHILD,
            can_replace_child: E::CAN_REPLACE_CHILD,
            default_important: E::DEFAULT_IMPORTANT,
            dynamic_artifact: E::dynamic_artifact(),
        }
    }
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    new_block_actions: Vec<(&'static str, Uuid, bool)>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            new_block_actions: Vec::new(),
        };
        registry.register_configurable::<audio::AudioEditor>();
        registry.register_creatable::<calendar::CalendarEditor>();
        registry.register_creatable::<database::DatabaseEditor>();
        registry.register_creatable::<database_schema::DatabaseSchemaEditor>();
        registry.register_creatable::<database_view::DatabaseViewEditor>();
        registry.register_creatable::<gui_builder::GuiBuilderEditor>();
        registry.register_configurable::<image::ImageEditor>();
        registry.register_creatable::<infinite_canvas::InfiniteCanvasEditor>();
        registry.register::<compiled_logic::CompiledLogicEditor>();
        #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "android"))]
        registry.register_counter_plugin();
        registry.register::<hotbar::HotbarEditor>();
        registry.register_creatable::<logic_game::LogicGameEditor>();
        registry.register_creatable::<logic_grid::LogicGridEditor>();
        registry.register_creatable::<map::MapEditor>();
        registry.register_creatable::<pixel_art::PixelArtEditor>();
        registry.register_creatable::<pixel_ray_tracer::PixelRayTracerEditor>();
        registry.register_creatable::<presentation::PresentationEditor>();
        registry.register_creatable::<scene_3d::Scene3DEditor>();
        registry.register::<settings::SettingsEditor>();
        registry.register_creatable::<text::TextEditor>();
        registry.register::<ui_settings::UiSettingsEditor>();
        registry.register_creatable::<version_control_data::VersionControlDataEditor>();
        registry.register::<version_control_worktree::VersionControlWorktreeEditor>();
        registry.register_creatable::<video::VideoEditor>();
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        registry.register_creatable::<browser_tab::WebBrowserTabEditor>();
        registry.register_creatable::<workspace_index::WorkspaceIndexEditor>();
        registry
    }

    /// Registers an editor for a block type that is only ever produced by
    /// another block, so it never appears in the new-block menu.
    fn register<E: EditorKind>(&mut self) {
        self.insert(EditorRegistration::of::<E>());
    }

    fn register_creatable<E: CreatableEditor>(&mut self) {
        let mut registration = EditorRegistration::of::<E>();
        registration.create = Some(CreateBlock::Immediate(|client| Box::new(E::create(client))));
        self.insert(registration);
    }

    fn register_configurable<E: ConfigurableEditor>(&mut self) {
        let mut registration = EditorRegistration::of::<E>();
        registration.create = Some(CreateBlock::Configured(|| {
            Box::new(EditorCreation::<E> {
                options: E::Options::default(),
            })
        }));
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

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "android"))]
    fn register_counter_plugin(&mut self) {
        use block_client::blocks::counter::Counter;
        use egui_material_icons::icons::ICON_123;

        let manifest = plugin::counter_manifest();
        manifest
            .validate()
            .expect("invalid built-in Counter plugin manifest");
        let display_name: &'static str = Box::leak(manifest.display_name.into_boxed_str());
        self.insert(EditorRegistration {
            block_type: Counter::TYPE_ID,
            display_name,
            icon: ICON_123,
            create: Some(CreateBlock::Immediate(|client| {
                Box::new(plugin::PluginEditor::new(
                    client,
                    client.create_block(Counter::new()),
                ))
            })),
            open: |client, id| {
                Box::new(plugin::PluginEditor::new(
                    client,
                    client.get_block::<Counter>(id),
                ))
            },
            can_add_child: false,
            can_delete_child: false,
            can_replace_child: false,
            default_important: false,
            dynamic_artifact: None,
        });
    }

    /// Creatable block types, with whether each belongs in the picker's main
    /// section.
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

    /// The functions that describe and rebuild artifacts made by `source_type`.
    pub(super) fn dynamic_artifact(
        &self,
        source_type: Uuid,
    ) -> Result<DynamicArtifactSupport, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        registration.dynamic_artifact.ok_or_else(|| {
            format!(
                "{} blocks do not generate dynamic artifacts",
                registration.display_name
            )
        })
    }

    pub fn regenerate_dynamic_artifact(
        &self,
        source_type: Uuid,
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
        let support = self.dynamic_artifact(source_type)?;
        (support.regenerate)(client, target_id, target_type, data)
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
