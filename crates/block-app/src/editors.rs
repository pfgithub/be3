#[cfg(not(target_os = "android"))]
mod browser_tab;
mod clipboard;
mod database;
mod database_schema;
pub(crate) mod image;
mod infinite_canvas;
mod map;
mod pixel_art;
mod pixel_ray_tracer;
mod presentation;
mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;
use std::hash::Hash;

use block::BlockParent;
use block_client::{
    blocks::workspace_index::BlockEntry, BlockClient, BlockHandleAccess, BlockHistoryHandle,
    BlockRelationships,
};
use eframe::egui;
use egui_material_icons::MaterialIcon;
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

pub struct EditorAccess<'a> {
    active: Vec<Uuid>,
    client: &'a BlockClient,
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
        client: &'a BlockClient,
        registry: &'a EditorRegistry,
        editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
    ) -> Self {
        Self {
            active: vec![active],
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
        self.active.push(id);
        let result = callback(editor.as_mut(), self);
        assert_eq!(self.active.pop(), Some(id));
        self.editors.insert(id, editor);
        Some(result)
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
        self.with_editor(id, |editor, editors| {
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
        self.with_editor(id, |editor, editors| {
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
        self.with_editor(id, |editor, editors| {
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
        self.with_editor(id, |editor, editors| {
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
        self.with_editor(id, |editor, editors| {
            editor.embedded_direct_editor_ui(ui, editors, scale, viewport)
        })?
    }

    pub fn set_parent(&mut self, id: Uuid, parent: BlockParent) -> bool {
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
    fn name(&self) -> String {
        self.block().name()
    }
    fn relationships(&self) -> Option<BlockRelationships> {
        self.block().relationships()
    }
    fn set_parent(&self, parent: BlockParent) {
        self.block().set_parent(parent);
    }
    fn add_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn delete_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn update(&mut self, _frame: &eframe::Frame) {}
    fn finish_frame(&mut self) {}
    fn set_tab_active(&mut self, _active: bool) {}
    fn tab_closed(&mut self) {}
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
            let next_action = editor.direct_editor_top_bar(ui, editors, &mut viewport);
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
                .default_pos(available.left_top() + egui::vec2(16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    let next_action = editor.direct_editor_left_sidebar(ui, editors);
                    if action.is_none() {
                        action = next_action;
                    }
                });
        }
        if editor.direct_editor_has_right_sidebar(editors) {
            egui::Window::new("Right sidebar")
                .id(egui::Id::new(("direct-editor-tab-right-window", id)))
                .pivot(egui::Align2::RIGHT_TOP)
                .default_width(240.0)
                .default_pos(available.right_top() + egui::vec2(-16.0, 16.0))
                .show(ui.ctx(), |ui| {
                    let next_action = editor.direct_editor_right_sidebar(ui, editors);
                    if action.is_none() {
                        action = next_action;
                    }
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
                    let next_action = editor.direct_editor_left_sidebar(ui, editors);
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
                    let next_action = editor.direct_editor_right_sidebar(ui, editors);
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
        let handles_viewport_input = editor.direct_editor_handles_viewport_input(editors);
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
                editor.direct_editor_ui(ui, editors, viewport_state.zoom, &mut viewport)
            })
            .inner;
        if action.is_none() {
            action = next_action;
        }

        if !handles_viewport_input
            && handle_direct_editor_background_input(
                ui.ctx(),
                viewport_rect,
                content_rect,
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
                let next_action = editor.direct_editor_ui(ui, editors, 1.0, &mut viewport);
                if action.is_none() {
                    action = next_action;
                }
            });
    }

    action
}

fn handle_direct_editor_background_input(
    context: &egui::Context,
    viewport_rect: egui::Rect,
    content_rect: egui::Rect,
    viewport: &mut DirectEditorTabViewport,
    max_zoom: f32,
) -> bool {
    let Some(pointer) = context
        .pointer_hover_pos()
        .filter(|pointer| viewport_rect.contains(*pointer) && !content_rect.contains(*pointer))
    else {
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
        registry.register(map::registration());
        registry.register(pixel_art::registration());
        registry.register(pixel_ray_tracer::registration());
        registry.register(presentation::registration());
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
