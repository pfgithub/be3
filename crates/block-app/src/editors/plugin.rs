use block_client::{blocks, blocks::workspace_index::BlockEntry, BlockClient, BlockHandleAccess};
use block_plugin_api::{
    BlockPick, BlockTypeDescriptor, ChildRect, CreationMode, EditorCapabilities, EditorInstanceId,
    EditorRegion, FrameChrome, FrameSpec, InteractionMode, PluginManifest, PresenceEntry,
    ResizeMode, ViewChange,
};
use eframe::egui;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

pub(crate) mod discovery;

use super::{
    embedded_editor_ui, frame_child_ui, paint_block_fallback, rect_corners, ArtifactSession,
    ArtifactStatus, BlockEditor, BlockRenderContext, CreationStep, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorResize, DirectEditorViewport, DirectEditorViewportCommand,
    DirectEditorViewportInput, EditorAccess, EditorAction, EditorRegistry, FrameSlot,
    PendingCreation,
};
use crate::{
    block_picker::BlockPicker,
    plugin_host::{
        ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, EditorView,
        HostChild, HostChildStatus, InstanceRole,
    },
};

fn child_interaction(editors: &EditorAccess<'_>, block_id: Uuid) -> InteractionMode {
    match editors.direct_editor_interaction(block_id) {
        Some(DirectEditorInteraction::Live) => InteractionMode::Live,
        Some(DirectEditorInteraction::Playback) => InteractionMode::Playback,
        Some(DirectEditorInteraction::Preview) | None => InteractionMode::Preview,
    }
}

fn rotated_corners(rect: egui::Rect, rotation: f32) -> [egui::Pos2; 4] {
    let corners = rect_corners(rect);
    if rotation == 0.0 {
        return corners;
    }
    let (sin, cos) = rotation.sin_cos();
    let center = rect.center();
    corners.map(|corner| {
        let offset = corner - center;
        center
            + egui::vec2(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            )
    })
}

fn collect_view_changes(
    child: block_plugin_api::ChildId,
    viewport: &mut DirectEditorViewport,
    views: &mut Vec<(block_plugin_api::ChildId, ViewChange)>,
) {
    for command in viewport.drain() {
        let change = match command {
            DirectEditorViewportCommand::Pan(delta) => ViewChange::Pan {
                x: delta.x,
                y: delta.y,
            },
            DirectEditorViewportCommand::Zoom { factor, anchor } => ViewChange::Zoom {
                factor,
                anchor: anchor.map(|anchor| (anchor.x, anchor.y)),
            },
            DirectEditorViewportCommand::Fit => ViewChange::Fit,
            DirectEditorViewportCommand::ResumeAutoFit => ViewChange::ResumeAutoFit,
            DirectEditorViewportCommand::AutoFit(_) => continue,
        };
        views.push((child, change));
    }
}

fn child_capabilities(editors: &EditorAccess<'_>, block_id: Uuid) -> EditorCapabilities {
    let Some(capabilities) = editors.direct_editor_capabilities(block_id) else {
        return EditorCapabilities::default();
    };
    EditorCapabilities {
        rotation: capabilities.allow_rotation,
        preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
        pan_and_zoom: capabilities.supports_pan_and_zoom,
    }
}

fn child_resize(editors: &EditorAccess<'_>, block_id: Uuid) -> ResizeMode {
    match editors.direct_editor_resize(block_id) {
        Some(DirectEditorResize::Horizontal) => ResizeMode::Horizontal,
        Some(DirectEditorResize::Vertical) => ResizeMode::Vertical,
        Some(DirectEditorResize::Both) => ResizeMode::Both,
        Some(DirectEditorResize::None) | None => ResizeMode::None,
    }
}

pub(super) fn block_type_descriptors(
    types: impl IntoIterator<Item = (uuid::Uuid, block_ui::BlockTypeEntry)>,
) -> Vec<BlockTypeDescriptor> {
    types
        .into_iter()
        .map(|(block_type, entry)| BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: entry.display_name,
            icon_codepoint: entry
                .icon
                .map(|icon| icon.codepoint.to_owned())
                .unwrap_or_default(),
        })
        .collect()
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

fn next_instance() -> EditorInstanceId {
    EditorInstanceId(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed))
}

pub(super) struct PluginCreation {
    plugin: Arc<PluginManifest>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    state: CreationState,
    committed: bool,
    block_pick: Option<PendingBlockPick>,
}

impl PluginCreation {
    pub(super) fn new(plugin: Arc<PluginManifest>) -> Self {
        Self {
            plugin,
            instance: next_instance(),
            context: None,
            state: CreationState::Starting,
            committed: false,
            block_pick: None,
        }
    }

    fn dialog_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Frame,
        )
        .map_or(CREATION_DIALOG_HEIGHT, |size| size.y.max(1.0));
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                role: InstanceRole::Creation,
                instance: self.instance,
                region: EditorRegion::Frame,
                frame: Some(FrameSpec::default()),
                size: egui::vec2(ui.available_width(), height),
                view: None,
            },
        )
        .present(ui);
    }
}

impl Drop for PluginCreation {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl PendingCreation for PluginCreation {
    fn ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> CreationStep {
        self.context = Some(ui.ctx().clone());
        if self.plugin.creation == CreationMode::Dialog {
            self.dialog_ui(ui, editors);
            serve_block_pick(
                &self.plugin.identity.id,
                self.instance,
                &mut self.block_pick,
                ui,
                editors,
                Vec::new(),
                block::BlockParent::Root,
            );
            let ready = crate::plugin_host::creation_ready(&self.plugin.identity.id, self.instance);
            if ready {
                self.state = CreationState::Ready;
            }
            return CreationStep::Options(ready);
        }
        self.state = crate::plugin_host::creation(
            ui.ctx(),
            CreationSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                instance: self.instance,
            },
        );
        CreationStep::Working
    }

    fn create(&mut self, client: &BlockClient) -> Result<Option<Box<dyn BlockEditor>>, String> {
        match &self.state {
            CreationState::Starting => return Ok(None),
            CreationState::Failed(error) => {
                return Err(format!(
                    "{} could not be created: {error}",
                    self.plugin.display_name
                ))
            }
            CreationState::Ready => {}
        }
        if !self.committed {
            self.committed = true;
            crate::plugin_host::commit_creation(&self.plugin.identity.id, self.instance);
        }
        match crate::plugin_host::take_created(&self.plugin.identity.id, self.instance) {
            None => Ok(None),
            Some(Ok(block_id)) => {
                let block_type = Uuid::from_bytes(self.plugin.block_type);
                let block = blocks::open(client, block_id, block_type)
                    .ok_or_else(|| format!("{block_type} is not a block type this app knows"))?;
                Ok(Some(Box::new(PluginEditor::new(
                    Arc::clone(&self.plugin),
                    block,
                ))))
            }
            Some(Err(error)) => {
                self.committed = false;
                Err(format!(
                    "{} could not be created: {error}",
                    self.plugin.display_name
                ))
            }
        }
    }
}

const CHILD_UNAVAILABLE: &str = "the block is already open above this editor";
const CREATION_DIALOG_HEIGHT: f32 = 96.0;
const PLUGIN_MIN_ZOOM: f32 = 1.0 / 64.0;

pub(super) struct PluginEditor {
    plugin: Arc<PluginManifest>,
    block: Box<dyn BlockHandleAccess>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    block_pick: Option<PendingBlockPick>,
    fullscreen: bool,
    active_this_frame: bool,
    main_region_id: Option<egui::Id>,
}

struct PendingBlockPick {
    request_id: u64,
    picker: BlockPicker,
}

fn serve_block_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
    pending: &mut Option<PendingBlockPick>,
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    excluded: Vec<Uuid>,
    parent: block::BlockParent,
) {
    if pending.is_none() {
        if let Some(request) = crate::plugin_host::take_block_pick(plugin_id, instance) {
            let mut picker = BlockPicker::default();
            if request.templates {
                picker.open_templates_for_types(excluded, request.block_types);
            } else {
                picker.open_for_types(excluded, request.block_types);
            }
            *pending = Some(PendingBlockPick {
                request_id: request.request_id,
                picker,
            });
        }
    }
    let Some(waiting) = pending.as_mut() else {
        return;
    };
    let picked = waiting.picker.handle(ui.ctx(), editors, parent);
    let pick = match picked {
        Some(result) => Some(BlockPick::Chosen {
            block_id: result.id.into_bytes(),
            block_type: result.block_type.into_bytes(),
        }),
        None if waiting.picker.is_open() => None,
        None => Some(BlockPick::Cancelled),
    };
    let Some(pick) = pick else {
        return;
    };
    let request_id = waiting.request_id;
    *pending = None;
    crate::plugin_host::block_picked(plugin_id, instance, request_id, pick);
}

impl PluginEditor {
    pub(super) fn new(plugin: Arc<PluginManifest>, block: Box<dyn BlockHandleAccess>) -> Self {
        Self {
            plugin,
            block,
            instance: next_instance(),
            context: None,
            block_pick: None,
            fullscreen: false,
            active_this_frame: false,
            main_region_id: None,
        }
    }

    fn presenting(&self) -> bool {
        crate::plugin_host::presenting(&self.plugin.identity.id, self.instance)
    }

    fn stop_presenting(&mut self) {
        let fullscreen = std::mem::take(&mut self.fullscreen);
        let Some(context) = self.context.clone() else {
            return;
        };
        if fullscreen {
            context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if self.presenting() {
            crate::plugin_host::present(&context, &self.plugin.identity.id, self.instance, false);
            context.request_repaint();
        }
    }

    fn presenting_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let screen = ui.ctx().content_rect();
        let entered = !std::mem::replace(&mut self.fullscreen, true);
        if entered {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
            self.stop_presenting();
            return None;
        }
        let area = egui::Id::new(("plugin-presenting", self.instance.0));
        let mut action = None;
        egui::Area::new(area)
            .order(egui::Order::Tooltip)
            .fixed_pos(screen.min)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(screen.size());
                ui.painter().rect_filled(screen, 0.0, egui::Color32::BLACK);
                action = self.frame_ui(ui, editors, FrameSpec::default(), screen.size(), None);
                if entered {
                    if let Some(id) = self.main_region_id {
                        ui.ctx().memory_mut(|memory| memory.request_focus(id));
                    }
                }
            });
        action
    }

    fn has_region(&self, region: EditorRegion) -> bool {
        self.plugin.regions.contains(&region)
    }

    fn frame_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
        chrome: bool,
    ) -> Option<EditorAction> {
        self.active_this_frame = true;
        self.context = Some(ui.ctx().clone());
        if self.presenting() {
            return self.presenting_ui(ui, editors);
        }
        self.stop_presenting();
        let rect = ui.available_rect_before_wrap();
        if self.plugin.capabilities.pan_and_zoom {
            viewport.auto_fit(self.block.id());
        }
        let view = self.plugin.capabilities.pan_and_zoom.then(|| EditorView {
            rect: viewport
                .content_rect()
                .unwrap_or(rect)
                .translate(-rect.min.to_vec2()),
            scale: viewport.scale(),
        });
        let frame = FrameSpec {
            chrome: match chrome {
                true => FrameChrome::Drawn,
                false => FrameChrome::None,
            },
            content: None,
            trail: Vec::new(),
        };
        let action = self.frame_ui(ui, editors, frame, rect.size(), view);
        self.take_view_changes(rect, viewport);
        action
    }

    fn frame_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        frame: FrameSpec,
        size: egui::Vec2,
        view: Option<EditorView>,
    ) -> Option<EditorAction> {
        self.region_ui(ui, editors, EditorRegion::Frame, Some(frame), size, view)
    }

    fn region_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        region: EditorRegion,
        frame: Option<FrameSpec>,
        size: egui::Vec2,
        view: Option<EditorView>,
    ) -> Option<EditorAction> {
        if !self.has_region(region) {
            return None;
        }
        self.context = Some(ui.ctx().clone());
        let presentation = crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                role: InstanceRole::Editor(EditorBlock {
                    id: self.block.id(),
                    block_type: self.block.block_type(),
                }),
                instance: self.instance,
                region,
                frame,
                size,
                view,
            },
        );
        if region == EditorRegion::Frame {
            self.main_region_id = presentation.id;
        }
        let mut action = presentation
            .open
            .map(|(id, block_type)| EditorAction::OpenBlock { id, block_type });
        let mut statuses = Vec::new();
        let mut views = Vec::new();
        let mut child_viewport = DirectEditorViewport::new();
        child_viewport.set_gestures_read(self.plugin.capabilities.pan_and_zoom);
        for child in presentation
            .children
            .iter()
            .filter(|child| child.is_below() && !child.frame_owner)
        {
            let next = self.child_ui(
                ui,
                editors,
                child,
                &mut child_viewport,
                &mut statuses,
                &mut views,
            );
            action = action.or(next);
        }
        presentation.present(ui);
        if region == EditorRegion::Frame {
            if let Some(rect) = presentation.loading_rect {
                let rect = rect.intersect(ui.clip_rect());
                let mut loading = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("plugin-loading", self.instance.0))
                        .max_rect(rect),
                );
                loading.set_clip_rect(rect);
                loading.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.weak("Loading plugin…");
                    });
                });
            }
        }
        for child in presentation
            .children
            .iter()
            .filter(|child| !child.is_below() || child.frame_owner)
        {
            let next = self.child_ui(
                ui,
                editors,
                child,
                &mut child_viewport,
                &mut statuses,
                &mut views,
            );
            action = action.or(next);
        }
        presentation.present_floating(ui);
        presentation.report(statuses);
        crate::plugin_host::report_child_views(
            &self.plugin.identity.id,
            self.instance,
            region,
            views,
        );
        if region == EditorRegion::Frame {
            self.block_pick_ui(ui, editors);
        }
        action
    }

    fn child_ui(
        &self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        child: &HostChild,
        viewport: &mut DirectEditorViewport,
        statuses: &mut Vec<HostChildStatus>,
        views: &mut Vec<(block_plugin_api::ChildId, ViewChange)>,
    ) -> Option<EditorAction> {
        editors.ensure(child.block_id, child.block_type);
        let available = editors.is_open(child.block_id);
        if let Some(size) = child.intrinsic {
            editors.set_direct_editor_intrinsic_size(child.block_id, size);
        }
        let hovered = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|position| child.rect.contains(position) && child.clip.contains(position));
        let mut action = None;
        let used: Option<egui::Vec2> = None;
        if available && child.is_preview() {
            let painter = ui.painter().with_clip_rect(child.clip);
            let rendered = editors.render(
                child.block_id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rotated_corners(child.rect, child.rotation),
                    opacity: child.opacity,
                },
            );
            if !rendered {
                paint_block_fallback(&painter, child.rect, None, editors);
            }
        } else if available && child.frame_owner && editors.is_frame_child(ui.ctx(), child.block_id)
        {
            action = frame_child_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-frame-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                viewport,
            );
            collect_view_changes(child.child, viewport, views);
        } else if available {
            action = embedded_editor_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                viewport,
            );
            collect_view_changes(child.child, viewport, views);
        }
        statuses.push(HostChildStatus {
            child: child.child,
            available,
            intrinsic: match used {
                Some(size) => Some(size),
                None => available
                    .then(|| editors.direct_editor_intrinsic_size(child.block_id))
                    .flatten(),
            },
            aspect_ratio: editors.preview_aspect_ratio(child.block_id),
            hovered,
            active: available && child.is_active(),
            interaction: child_interaction(editors, child.block_id),
            capabilities: child_capabilities(editors, child.block_id),
            resize: child_resize(editors, child.block_id),
            error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
        });
        action
    }

    fn preview_children_ui(
        &self,
        painter: &egui::Painter,
        editors: &mut EditorAccess<'_>,
        presentation: &crate::plugin_host::PreviewPresentation,
        corners: [egui::Pos2; 4],
        opacity: f32,
    ) {
        if presentation.children.is_empty() {
            return;
        }
        let mut statuses = Vec::new();
        for child in &presentation.children {
            editors.ensure(child.block_id, child.block_type);
            let available = editors.is_open(child.block_id);
            if available && child.is_preview() && presentation.drawn {
                let corners = mapped_corners(corners, presentation.size, child.rect);
                editors.render(
                    child.block_id,
                    BlockRenderContext {
                        painter,
                        corners,
                        opacity,
                    },
                );
            }
            statuses.push(HostChildStatus {
                child: child.child,
                available,
                intrinsic: available
                    .then(|| editors.direct_editor_intrinsic_size(child.block_id))
                    .flatten(),
                aspect_ratio: editors.preview_aspect_ratio(child.block_id),
                hovered: false,
                active: false,
                interaction: child_interaction(editors, child.block_id),
                capabilities: child_capabilities(editors, child.block_id),
                resize: child_resize(editors, child.block_id),
                error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
            });
        }
        crate::plugin_host::report_children(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Preview,
            statuses,
        );
    }

    fn block_pick_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        serve_block_pick(
            &self.plugin.identity.id,
            self.instance,
            &mut self.block_pick,
            ui,
            editors,
            vec![self.block.id()],
            block::BlockParent::Uuid(self.block.id()),
        );
    }

    fn take_view_changes(&mut self, rect: egui::Rect, viewport: &mut DirectEditorViewport) {
        if !self.plugin.capabilities.pan_and_zoom {
            return;
        }
        for change in crate::plugin_host::take_view_changes(&self.plugin.identity.id, self.instance)
        {
            match change {
                ViewChange::Pan { x, y } => viewport.pan(egui::vec2(x, y)),
                ViewChange::Zoom { factor, anchor } => {
                    viewport.change_zoom(factor, anchor.map(|(x, y)| rect.min + egui::vec2(x, y)))
                }
                ViewChange::Fit => viewport.fit(),
                ViewChange::ResumeAutoFit => viewport.resume_auto_fit(),
            }
        }
    }

    fn close(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl Drop for PluginEditor {
    fn drop(&mut self) {
        self.stop_presenting();
        self.close();
    }
}

impl BlockEditor for PluginEditor {
    fn block(&self) -> &dyn BlockHandleAccess {
        self.block.as_ref()
    }

    fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        if !self.plugin.children.replace {
            return None;
        }
        match crate::plugin_host::replace_child(
            &self.plugin.identity.id,
            self.instance,
            old,
            new.id,
        ) {
            Some(true) => Some(true),
            Some(false) => self.block.replace_child(old, new.id),
            None => None,
        }
    }

    fn sync_cursor_presence(&mut self, client: &BlockClient, visible: bool) {
        let block_id = self.block.id();
        let entries = client
            .presence_entries(block_id)
            .into_iter()
            .map(|(client_id, presence_id, data)| PresenceEntry {
                client_id,
                presence_id: presence_id.into_bytes(),
                data,
            })
            .collect();
        let published =
            crate::plugin_host::presence(&self.plugin.identity.id, self.instance, visible, entries);
        for (presence_id, data) in published {
            client.set_presence_data(block_id, presence_id, data);
        }
    }

    fn reveal_presence_cursor(&mut self, client_id: block::ClientId) {
        crate::plugin_host::reveal_presence(&self.plugin.identity.id, self.instance, client_id);
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        if !self.has_region(EditorRegion::Preview) {
            return false;
        }
        self.context = Some(context.painter.ctx().clone());
        let presentation = crate::plugin_host::preview(
            context.painter,
            crate::plugin_host::PreviewSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                block_id: self.block.id(),
                block_type: self.block.block_type(),
                instance: self.instance,
                corners: context.corners,
                opacity: context.opacity,
            },
        );
        self.preview_children_ui(
            context.painter,
            editors,
            &presentation,
            context.corners,
            context.opacity,
        );
        presentation.drawn
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        crate::plugin_host::aspect_ratio(&self.plugin.identity.id, self.instance)
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: self.plugin.capabilities.rotation,
            preserve_aspect_ratio: self.plugin.capabilities.preserve_aspect_ratio,
            supports_pan_and_zoom: self.plugin.capabilities.pan_and_zoom,
        }
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        self.plugin.capabilities.pan_and_zoom
    }

    fn direct_editor_min_zoom(&self) -> f32 {
        PLUGIN_MIN_ZOOM
    }

    fn direct_editor_viewport_input(
        &self,
        _editors: &EditorAccess<'_>,
    ) -> DirectEditorViewportInput {
        if self.plugin.capabilities.pan_and_zoom {
            DirectEditorViewportInput::Viewport
        } else {
            DirectEditorViewportInput::Background
        }
    }

    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        match self.plugin.interaction {
            InteractionMode::Preview => DirectEditorInteraction::Preview,
            InteractionMode::Live => DirectEditorInteraction::Live,
            InteractionMode::Playback => DirectEditorInteraction::Playback,
        }
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        match self.plugin.resize {
            ResizeMode::None => DirectEditorResize::None,
            ResizeMode::Horizontal => DirectEditorResize::Horizontal,
            ResizeMode::Vertical => DirectEditorResize::Vertical,
            ResizeMode::Both => DirectEditorResize::Both,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        Some(
            crate::plugin_host::intrinsic_size(&self.plugin.identity.id, self.instance)
                .unwrap_or_else(|| egui::vec2(420.0, 240.0)),
        )
    }

    fn set_direct_editor_intrinsic_size(
        &mut self,
        size: egui::Vec2,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        crate::plugin_host::resized(&self.plugin.identity.id, self.instance, size);
        false
    }

    fn direct_editor_owns_frame(&self) -> bool {
        true
    }

    fn direct_editor_frame_child(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Uuid> {
        crate::plugin_host::frame_child(&self.plugin.identity.id, self.instance)
    }

    fn clear_direct_editor_frame_child(&mut self, _editors: &mut EditorAccess<'_>) {
        crate::plugin_host::revoke_frame_child(&self.plugin.identity.id, self.instance);
    }

    fn take_direct_editor_frame_exit(&mut self) -> bool {
        crate::plugin_host::take_leaving(&self.plugin.identity.id, self.instance)
    }

    fn direct_editor_frame_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        slot: &FrameSlot,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.active_this_frame = true;
        self.context = Some(ui.ctx().clone());
        if self.presenting() {
            return self.presenting_ui(ui, editors);
        }
        self.stop_presenting();
        let rect = slot.frame;
        if self.plugin.capabilities.pan_and_zoom {
            viewport.auto_fit(self.block.id());
        }
        let view = self.plugin.capabilities.pan_and_zoom.then(|| EditorView {
            rect: viewport
                .content_rect()
                .unwrap_or(rect)
                .translate(-rect.min.to_vec2()),
            scale: viewport.scale(),
        });
        let frame = FrameSpec {
            chrome: match slot.chrome {
                block_ui::frame::Chrome::Drawn => FrameChrome::Drawn,
                block_ui::frame::Chrome::Reserved => FrameChrome::Reserved,
                block_ui::frame::Chrome::None => FrameChrome::None,
            },
            content: slot.content.map(|content| {
                let content = content.translate(-rect.min.to_vec2());
                ChildRect {
                    x: content.min.x,
                    y: content.min.y,
                    width: content.width(),
                    height: content.height(),
                }
            }),
            trail: slot.trail.clone(),
        };
        let action = self.frame_ui(ui, editors, frame, rect.size(), view);
        self.take_view_changes(rect, viewport);
        action
    }

    fn direct_editor_viewport_rect(&self, frame: egui::Rect) -> egui::Rect {
        crate::plugin_host::frame_rects(&self.plugin.identity.id, self.instance)
            .map(|rects| rects.content.translate(frame.min.to_vec2()))
            .filter(|content| content.is_positive())
            .unwrap_or(frame)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.frame_editor_ui(ui, editors, viewport, false)
    }

    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.frame_editor_ui(ui, editors, viewport, false)
    }

    fn set_tab_active(&mut self, active: bool) {
        if active {
            self.active_this_frame = true;
        }
    }

    fn finish_frame(&mut self) {
        if !std::mem::take(&mut self.active_this_frame) {
            self.stop_presenting();
        }
    }

    fn tab_closed(&mut self) {
        self.stop_presenting();
        self.close();
    }
}

pub(super) struct PluginArtifact {
    plugin: Arc<PluginManifest>,
    block: EditorBlock,
    client_id: Uuid,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    resync: bool,
    outcome: Option<Result<(), String>>,
    regenerating: bool,
}

impl PluginArtifact {
    pub(super) fn new(
        plugin: Arc<PluginManifest>,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Self {
        Self {
            plugin,
            client_id,
            block: EditorBlock {
                id: target_id,
                block_type: target_type,
            },
            instance: next_instance(),
            context: None,
            resync: false,
            outcome: None,
            regenerating: false,
        }
    }
}

impl Drop for PluginArtifact {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl ArtifactSession for PluginArtifact {
    fn poll(
        &mut self,
        ctx: &egui::Context,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        data: &[u8],
    ) -> ArtifactStatus {
        self.context = Some(ctx.clone());
        let state = crate::plugin_host::artifact(
            ctx,
            ArtifactSlot {
                plugin: &self.plugin,
                block_types: registry.plugin_block_types(),
                client: Arc::clone(client),
                client_id: self.client_id,
                instance: self.instance,
                block: self.block,
                data,
                resync: std::mem::take(&mut self.resync),
            },
        );
        if let Some(outcome) =
            crate::plugin_host::take_artifact_outcome(&self.plugin.identity.id, self.instance)
        {
            self.regenerating = false;
            self.outcome = Some(outcome);
        }
        match state {
            ArtifactState::Starting => ArtifactStatus::Starting,
            ArtifactState::Described { source, summary } => {
                ArtifactStatus::Described { source, summary }
            }
            ArtifactState::Failed(error) => ArtifactStatus::Failed(error),
        }
    }

    fn settings_ui(
        &mut self,
        ui: &mut egui::Ui,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        draft: &mut Vec<u8>,
    ) {
        self.context = Some(ui.ctx().clone());
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::ArtifactSettings,
        )
        .map_or(ARTIFACT_SETTINGS_HEIGHT, |size| size.y.max(1.0));
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: registry.plugin_block_types(),
                client: Arc::clone(client),
                client_id: self.client_id,
                role: InstanceRole::Artifact(self.block),
                instance: self.instance,
                region: EditorRegion::ArtifactSettings,
                frame: None,
                size: egui::vec2(ui.available_width(), height),
                view: None,
            },
        )
        .present(ui);
        if let Some(edited) =
            crate::plugin_host::artifact_draft(&self.plugin.identity.id, self.instance)
        {
            *draft = edited;
        }
    }

    fn summary(&self, _draft: &[u8]) -> Option<String> {
        None
    }

    fn cancel_settings(&mut self) {
        self.resync = true;
    }

    fn regenerate(&mut self, _client: &Arc<BlockClient>, data: &[u8]) {
        self.outcome = None;
        self.regenerating = true;
        crate::plugin_host::regenerate_artifact(&self.plugin.identity.id, self.instance, data);
    }

    fn take_outcome(&mut self) -> Option<Result<(), String>> {
        self.outcome.take()
    }

    fn regenerating(&self) -> bool {
        self.regenerating
    }
}

const ARTIFACT_SETTINGS_HEIGHT: f32 = 72.0;

fn mapped_corners(corners: [egui::Pos2; 4], size: egui::Vec2, rect: egui::Rect) -> [egui::Pos2; 4] {
    let horizontal = corners[1] - corners[0];
    let vertical = corners[3] - corners[0];
    let at = |x: f32, y: f32| {
        corners[0]
            + horizontal * (x / size.x.max(f32::EPSILON))
            + vertical * (y / size.y.max(f32::EPSILON))
    };
    [
        at(rect.min.x, rect.min.y),
        at(rect.max.x, rect.min.y),
        at(rect.max.x, rect.max.y),
        at(rect.min.x, rect.max.y),
    ]
}
