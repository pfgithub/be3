pub(crate) mod plugin;
mod unsupported;

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use block::{BlockAccess, BlockParent, BlockReference};
use block_client::{
    blocks::{self, workspace_index::BlockEntry},
    BlockClient, BlockHandleAccess, BlockHistoryHandle, BlockRelationships,
};
use block_plugin_api::PluginManifest;
pub(super) use block_ui::{paint_name, BlockLabel};
use block_ui::{BlockTypeEntry, BlockTypes};
use eframe::egui;
use egui_material_icons::{icons::ICON_LOCK, MaterialIcon};
use uuid::Uuid;

use self::unsupported::UnsupportedEditor;

const DIRECT_EDITOR_MIN_ZOOM: f32 = 0.25;
const DIRECT_EDITOR_MAX_ZOOM: f32 = 32.0;
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
    commands: Vec<DirectEditorViewportCommand>,
    content_rect: Option<egui::Rect>,
}

impl DirectEditorViewport {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            content_rect: None,
        }
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

    pub fn resume_auto_fit(&mut self) {
        self.commands
            .push(DirectEditorViewportCommand::ResumeAutoFit);
    }

    pub fn auto_fit(&mut self, target: Uuid) {
        self.commands
            .push(DirectEditorViewportCommand::AutoFit(target));
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
        block_ui::frame::read_only_scope(ui, !access.can_edit(), |ui| {
            self.with_editor(id, |editor, editors| callback(editor, editors, ui))
        })
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

    pub fn set_direct_editor_intrinsic_size(&mut self, id: Uuid, size: egui::Vec2) -> bool {
        if !self.access_for(id).can_edit() {
            return false;
        }
        self.with_editor(id, |editor, editors| {
            editor.set_direct_editor_intrinsic_size(size, editors)
        })
        .unwrap_or(false)
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

    pub fn block_label(&self, id: Uuid) -> String {
        self.client
            .cached_block(id)
            .map(|cached| BlockLabel::for_cached(self.registry, &cached).name)
            .unwrap_or_else(|| "Block".to_owned())
    }

    pub fn direct_editor_frame_child(&mut self, id: Uuid) -> Option<Uuid> {
        self.with_editor(id, |editor, editors| {
            editor.direct_editor_frame_child(editors)
        })?
    }

    pub fn is_frame_child(&self, context: &egui::Context, id: Uuid) -> bool {
        tab_frame(context).is_some_and(|tab| tab.stack.contains(&id))
    }

    pub fn clear_direct_editor_frame_child(&mut self, id: Uuid) {
        self.with_editor(id, |editor, editors| {
            editor.clear_direct_editor_frame_child(editors);
        });
    }

    fn direct_editor_frame_ui(
        &mut self,
        id: Uuid,
        ui: &mut egui::Ui,
        id_salt: impl Hash,
        slot: &FrameSlot,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(id_salt)
                .max_rect(slot.frame)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(slot.clip);
        let access = self.access_for(id);
        if !access.can_view() {
            no_access_notice(&mut child);
            return None;
        }
        let (action, exit) = self
            .with_editor(id, |editor, editors| {
                direct_editor_frame_ui(editor, &mut child, editors, slot, Some(viewport))
            })
            .unwrap_or((None, false));
        if exit {
            request_frame_exit(ui.ctx());
        }
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
    fn set_direct_editor_intrinsic_size(
        &mut self,
        _size: egui::Vec2,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        false
    }
    fn direct_editor_owns_frame(&self) -> bool {
        false
    }
    fn direct_editor_frame_child(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Uuid> {
        None
    }
    fn clear_direct_editor_frame_child(&mut self, _editors: &mut EditorAccess<'_>) {}
    fn take_direct_editor_frame_exit(&mut self) -> bool {
        false
    }
    fn direct_editor_frame_ui(
        &mut self,
        _ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _slot: &FrameSlot,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        None
    }
    fn direct_editor_viewport_rect(&self, frame: egui::Rect) -> egui::Rect {
        frame
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

#[derive(Clone)]
pub struct FrameSlot {
    pub frame: egui::Rect,
    pub clip: egui::Rect,
    pub content: Option<egui::Rect>,
    pub chrome: block_ui::frame::Chrome,
    pub trail: Vec<String>,
}

#[derive(Clone)]
struct TabFrame {
    frame: egui::Rect,
    clip: egui::Rect,
    stack: Vec<Uuid>,
    trail: Vec<String>,
}

fn tab_frame_id() -> egui::Id {
    egui::Id::new("direct-editor-frame-stack")
}

fn tab_frame(context: &egui::Context) -> Option<TabFrame> {
    context.data(|data| data.get_temp::<TabFrame>(tab_frame_id()))
}

fn frame_exit_id() -> egui::Id {
    egui::Id::new("direct-editor-frame-exit")
}

fn request_frame_exit(context: &egui::Context) {
    context.data_mut(|data| data.insert_temp(frame_exit_id(), true));
    context.request_repaint();
}

fn take_frame_exit(context: &egui::Context) -> bool {
    context.data_mut(|data| {
        let requested = data.get_temp::<bool>(frame_exit_id()).unwrap_or_default();
        data.remove::<bool>(frame_exit_id());
        requested
    })
}

pub fn direct_editor_tab_ui(
    editor: &mut dyn BlockEditor,
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
) -> Option<EditorAction> {
    let frame = ui.available_rect_before_wrap();
    let clip = frame.intersect(ui.clip_rect());
    let mut stack = Vec::new();
    let mut trail = vec![editors.block_label(editor.id())];
    let mut child = editor.direct_editor_frame_child(editors);
    while let Some(id) = child {
        if stack.contains(&id) {
            break;
        }
        stack.push(id);
        trail.push(editors.block_label(id));
        child = editors.direct_editor_frame_child(id);
    }
    let owner = stack.last().copied();
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            tab_frame_id(),
            TabFrame {
                frame,
                clip,
                stack: stack.clone(),
                trail,
            },
        );
    });
    let slot = FrameSlot {
        frame,
        clip,
        content: None,
        chrome: match owner {
            Some(_) => block_ui::frame::Chrome::Reserved,
            None => block_ui::frame::Chrome::Drawn,
        },
        trail: Vec::new(),
    };
    let (action, own_exit) = direct_editor_frame_ui(editor, ui, editors, &slot, None);
    let exit = own_exit || take_frame_exit(ui.ctx());
    if exit {
        match stack.len() {
            0 | 1 => editor.clear_direct_editor_frame_child(editors),
            depth => {
                let parent = stack[depth - 2];
                editors.clear_direct_editor_frame_child(parent);
            }
        }
        ui.ctx().request_repaint();
    }
    action
}

pub fn frame_child_ui(
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    id_salt: impl Hash,
    content: egui::Rect,
    clip_rect: egui::Rect,
    viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let tab = tab_frame(ui.ctx())?;
    let depth = tab.stack.iter().position(|id| *id == block_id)?;
    let slot = FrameSlot {
        frame: tab.frame,
        clip: tab.clip,
        content: Some(content.intersect(clip_rect)),
        chrome: match depth + 1 == tab.stack.len() {
            true => block_ui::frame::Chrome::Drawn,
            false => block_ui::frame::Chrome::Reserved,
        },
        trail: tab.trail[..depth + 2].to_vec(),
    };
    let previous = viewport.replace_content_rect(Some(content));
    let action = editors.direct_editor_frame_ui(block_id, ui, id_salt, &slot, viewport);
    viewport.replace_content_rect(previous);
    action
}

pub(crate) fn direct_editor_frame_ui(
    editor: &mut dyn BlockEditor,
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    slot: &FrameSlot,
    outer: Option<&mut DirectEditorViewport>,
) -> (Option<EditorAction>, bool) {
    let id = editor.id();
    let read_only = !editors.access().can_edit();
    let viewport_id = egui::Id::new(("direct-editor-tab-viewport", id));
    let viewport_state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<DirectEditorTabViewport>(viewport_id))
        .unwrap_or_default();
    let owns_frame = editor.direct_editor_owns_frame();
    let chrome = match owns_frame {
        true => block_ui::frame::Chrome::None,
        false => slot.chrome,
    };
    let has_left_sidebar = !owns_frame && editor.direct_editor_has_left_sidebar(editors);
    let has_right_sidebar = !owns_frame && editor.direct_editor_has_right_sidebar(editors);
    let drawn = chrome == block_ui::frame::Chrome::Drawn;
    let mut bands = DirectEditorTabBands {
        id,
        owns_frame,
        slot: slot.clone(),
        viewport_id,
        capabilities: editor.direct_editor_capabilities(),
        max_zoom: editor.direct_editor_max_zoom(),
        min_zoom: editor.direct_editor_min_zoom(),
        read_only,
        viewport: DirectEditorViewport::new(),
        viewport_state,
        editor,
        editors,
        outer,
        exit: false,
        action: None,
    };
    let mut host = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("direct-editor-frame", id))
            .max_rect(slot.frame)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    host.set_clip_rect(slot.clip);
    let outcome = block_ui::frame::Frame::new(egui::Id::new(("direct-editor-tab", id)))
        .chrome(chrome)
        .toolbar(!owns_frame)
        .left_sidebar(has_left_sidebar)
        .right_sidebar(has_right_sidebar)
        .read_only(read_only)
        .content(match owns_frame {
            true => None,
            false => slot.content,
        })
        .trail(match drawn {
            true => slot.trail.clone(),
            false => Vec::new(),
        })
        .show(&mut host, &mut bands);
    let exit = outcome.exit || bands.exit;
    (bands.action, exit)
}

struct DirectEditorTabBands<'a, 'b> {
    id: Uuid,
    owns_frame: bool,
    slot: FrameSlot,
    exit: bool,
    viewport_id: egui::Id,
    capabilities: DirectEditorCapabilities,
    max_zoom: f32,
    min_zoom: f32,
    read_only: bool,
    viewport: DirectEditorViewport,
    viewport_state: DirectEditorTabViewport,
    editor: &'a mut dyn BlockEditor,
    editors: &'a mut EditorAccess<'b>,
    outer: Option<&'a mut DirectEditorViewport>,
    action: Option<EditorAction>,
}

impl DirectEditorTabBands<'_, '_> {
    fn draw(&mut self, ui: &mut egui::Ui, zoom: f32) -> Option<EditorAction> {
        let read_only = self.read_only;
        let owns_frame = self.owns_frame;
        let slot = self.slot.clone();
        let viewport = match &mut self.outer {
            Some(outer) => outer,
            None => &mut self.viewport,
        };
        let editor = &mut *self.editor;
        let editors = &mut *self.editors;
        let action = block_ui::frame::read_only_scope(ui, read_only, |ui| match owns_frame {
            true => editor.direct_editor_frame_ui(ui, editors, &slot, viewport),
            false => editor.direct_editor_ui(ui, editors, zoom, viewport),
        });
        if owns_frame {
            self.exit |= self.editor.take_direct_editor_frame_exit();
        }
        action
    }

    fn child_content_ui(&mut self, ui: &mut egui::Ui) {
        let band = ui.available_rect_before_wrap();
        let rect = match self.owns_frame {
            true => self.slot.frame,
            false => band,
        };
        let action = ui
            .new_child(
                egui::UiBuilder::new()
                    .id_salt(("direct-editor-frame-content", self.id))
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .scope(|ui| {
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                ui.set_min_size(rect.size());
                self.draw(ui, 1.0)
            })
            .inner;
        self.record(action);
        let input = self.editor.direct_editor_viewport_input(self.editors);
        if input == DirectEditorViewportInput::Viewport {
            let viewport = match &mut self.outer {
                Some(outer) => outer,
                None => &mut self.viewport,
            };
            viewport_gesture_input(ui.ctx(), band.intersect(ui.clip_rect()), None, viewport);
        }
    }

    fn record(&mut self, action: Option<EditorAction>) {
        if self.action.is_none() {
            self.action = action;
        }
    }
}

impl block_ui::frame::FrameBands for DirectEditorTabBands<'_, '_> {
    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let action = self
            .editor
            .direct_editor_top_bar(ui, self.editors, &mut self.viewport);
        self.record(action);
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let action = self.editor.direct_editor_left_sidebar(ui, self.editors);
        self.record(action);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let action = self.editor.direct_editor_right_sidebar(ui, self.editors);
        self.record(action);
    }

    fn content_ui(&mut self, ui: &mut egui::Ui) {
        if self.outer.is_some() {
            self.child_content_ui(ui);
            return;
        }
        let id = self.id;
        let band = ui.available_rect_before_wrap();
        let viewport_size = self
            .editor
            .direct_editor_viewport_rect(band)
            .size()
            .max(egui::Vec2::splat(1.0));
        let intrinsic_size = self
            .editor
            .direct_editor_intrinsic_size(self.editors)
            .unwrap_or_default();
        let content_size = egui::vec2(
            viewport_size.x.max(intrinsic_size.x),
            viewport_size.y.max(intrinsic_size.y),
        );
        if !self.capabilities.supports_pan_and_zoom {
            let action = match self.owns_frame {
                true => self.draw(ui, 1.0),
                false => {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_size(content_size);
                            self.draw(ui, 1.0)
                        })
                        .inner
                }
            };
            self.record(action);
            return;
        }
        let (allocated, _) = ui.allocate_exact_size(band.size(), egui::Sense::hover());
        let viewport_rect = self.editor.direct_editor_viewport_rect(allocated);
        if let Some(previous_center) = self.viewport_state.center.replace(viewport_rect.center()) {
            self.viewport_state.pan += previous_center - viewport_rect.center();
        }
        let transformed_size = content_size * self.viewport_state.zoom;
        let content_rect = egui::Rect::from_center_size(
            viewport_rect.center() + self.viewport_state.pan,
            transformed_size,
        );
        self.viewport.replace_content_rect(Some(content_rect));
        let fills_viewport = self.editor.direct_editor_fills_viewport();

        let mut viewport_input = self.editor.direct_editor_viewport_input(self.editors);
        if self.read_only && viewport_input == DirectEditorViewportInput::Editor {
            viewport_input = DirectEditorViewportInput::Background;
        }
        let editor_rect = match (self.owns_frame, fills_viewport) {
            (true, _) => allocated,
            (false, true) => viewport_rect,
            (false, false) => content_rect,
        };
        let zoom = self.viewport_state.zoom;
        let action = ui
            .new_child(
                egui::UiBuilder::new()
                    .id_salt(("direct-editor-tab-content", id))
                    .max_rect(editor_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .scope(|ui| {
                ui.set_clip_rect(editor_rect.intersect(ui.clip_rect()));
                ui.set_min_size(editor_rect.size());
                self.draw(ui, zoom)
            })
            .inner;
        self.record(action);

        match viewport_input {
            DirectEditorViewportInput::Editor => {}
            DirectEditorViewportInput::Background => viewport_gesture_input(
                ui.ctx(),
                viewport_rect,
                (!self.read_only).then_some(content_rect),
                &mut self.viewport,
            ),
            DirectEditorViewportInput::Viewport => {
                viewport_gesture_input(ui.ctx(), viewport_rect, None, &mut self.viewport)
            }
        }

        let commands: Vec<_> = self.viewport.drain().collect();
        for command in commands {
            match command {
                DirectEditorViewportCommand::Pan(delta) => {
                    self.viewport_state.pan += delta;
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Zoom { factor, anchor } => {
                    let old_zoom = self.viewport_state.zoom;
                    let new_zoom = (old_zoom * factor).clamp(self.min_zoom, self.max_zoom);
                    if new_zoom != old_zoom {
                        let anchor = anchor.unwrap_or_else(|| viewport_rect.center());
                        self.viewport_state.pan = (anchor - viewport_rect.center())
                            - ((anchor - viewport_rect.center()) - self.viewport_state.pan)
                                * (new_zoom / old_zoom);
                        self.viewport_state.zoom = new_zoom;
                    }
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Fit => {
                    fit_direct_editor_viewport(
                        &mut self.viewport_state,
                        viewport_size,
                        content_size,
                        self.min_zoom,
                    );
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::AutoFit(target) => {
                    let auto_fit = self.viewport_state.auto_fit.get_or_insert(AutoFitState {
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
                            &mut self.viewport_state,
                            viewport_size,
                            content_size,
                            self.min_zoom,
                        );
                    }
                }
                DirectEditorViewportCommand::ResumeAutoFit => {
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = true;
                        fit_direct_editor_viewport(
                            &mut self.viewport_state,
                            viewport_size,
                            content_size,
                            self.min_zoom,
                        );
                    }
                }
            }
        }
        let state = self.viewport_state;
        ui.ctx()
            .data_mut(|data| data.insert_temp(self.viewport_id, state));
    }
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

type OpenEditor = Box<dyn Fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>>;
type CreateOptions = Box<dyn Fn() -> Box<dyn PendingCreation>>;

struct ArtifactProvider(Arc<PluginManifest>);

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

pub(super) trait PendingCreation {
    fn ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> CreationStep;
    fn create(&mut self, client: &BlockClient) -> Result<Option<Box<dyn BlockEditor>>, String>;
}

pub(super) enum CreationStep {
    Options(bool),
    Working,
}

struct CreateBlock(CreateOptions);

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
                    Some(CreateBlock(Box::new(move || {
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
                .then(|| ArtifactProvider(Arc::clone(&manifest))),
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
            Some(ArtifactProvider(manifest)) => Ok(Box::new(plugin::PluginArtifact::new(
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

    pub(super) fn create(&self, block_type: Uuid) -> Option<Box<dyn PendingCreation>> {
        let CreateBlock(options) = self.registrations.get(&block_type)?.create.as_ref()?;
        Some(options())
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
