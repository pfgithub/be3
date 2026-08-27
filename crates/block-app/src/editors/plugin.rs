use block_client::{blocks, BlockClient, BlockHandleAccess};
use block_plugin_api::{
    BlockPick, BlockTypeDescriptor, CreationMode, EditorInstanceId, EditorRegion, InteractionMode,
    PluginManifest, ResizeMode, ViewChange,
};
use eframe::egui;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

pub(crate) mod discovery;

use super::{
    embedded_editor_ui, paint_block_fallback, rect_corners, ArtifactSession, ArtifactStatus,
    BlockEditor, BlockRenderContext, CreationStep, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorResize, DirectEditorViewport, DirectEditorViewportInput,
    EditorAccess, EditorAction, EditorRegistry, PendingCreation,
};
use crate::{
    block_picker::BlockPicker,
    plugin_host::{
        ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, HostChild,
        HostChildStatus, InstanceRole,
    },
};

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
}

impl PluginCreation {
    pub(super) fn new(plugin: Arc<PluginManifest>) -> Self {
        Self {
            plugin,
            instance: next_instance(),
            context: None,
            state: CreationState::Starting,
            committed: false,
        }
    }

    fn dialog_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Main,
        )
        .map_or(CREATION_DIALOG_HEIGHT, |size| size.y.max(1.0));
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                role: InstanceRole::Creation,
                instance: self.instance,
                region: EditorRegion::Main,
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
                action = self.region_ui(ui, editors, EditorRegion::Main, screen.size(), None);
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

    fn region_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        region: EditorRegion,
        size: egui::Vec2,
        view: Option<egui::Rect>,
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
                role: InstanceRole::Editor(EditorBlock {
                    id: self.block.id(),
                    block_type: self.block.block_type(),
                }),
                instance: self.instance,
                region,
                size,
                view,
            },
        );
        if region == EditorRegion::Main {
            self.main_region_id = presentation.id;
        }
        let mut action = presentation
            .open
            .map(|(id, block_type)| EditorAction::OpenBlock { id, block_type });
        let mut statuses = Vec::new();
        let mut child_viewport = DirectEditorViewport::new(1.0);
        for child in presentation
            .children
            .iter()
            .filter(|child| child.is_below())
        {
            let next = self.child_ui(ui, editors, child, &mut child_viewport, &mut statuses);
            action = action.or(next);
        }
        presentation.present(ui);
        for child in presentation
            .children
            .iter()
            .filter(|child| !child.is_below())
        {
            let next = self.child_ui(ui, editors, child, &mut child_viewport, &mut statuses);
            action = action.or(next);
        }
        presentation.report(statuses);
        if region == EditorRegion::Main {
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
    ) -> Option<EditorAction> {
        editors.ensure(child.block_id, child.block_type);
        let available = editors.is_open(child.block_id);
        let hovered = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|position| child.rect.contains(position) && child.clip.contains(position));
        let mut action = None;
        if available && child.is_preview() {
            let painter = ui.painter().with_clip_rect(child.clip);
            let rendered = editors.render(
                child.block_id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rect_corners(child.rect),
                    opacity: 1.0,
                },
            );
            if !rendered {
                paint_block_fallback(&painter, child.rect, None, editors);
            }
        } else if available {
            action = embedded_editor_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                1.0,
                viewport,
            );
            viewport.drain().for_each(drop);
        }
        statuses.push(HostChildStatus {
            child: child.child,
            available,
            intrinsic: available
                .then(|| editors.direct_editor_intrinsic_size(child.block_id))
                .flatten(),
            aspect_ratio: editors.preview_aspect_ratio(child.block_id),
            hovered,
            active: available && child.is_active(),
            error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
        });
        action
    }

    fn block_pick_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        let plugin_id = &self.plugin.identity.id;
        if self.block_pick.is_none() {
            if let Some(request) = crate::plugin_host::take_block_pick(plugin_id, self.instance) {
                let mut picker = BlockPicker::default();
                if request.templates {
                    picker.open_templates_for_types([self.block.id()], request.block_types);
                } else {
                    picker.open_for_types([self.block.id()], request.block_types);
                }
                self.block_pick = Some(PendingBlockPick {
                    request_id: request.request_id,
                    picker,
                });
            }
        }
        let Some(pending) = &mut self.block_pick else {
            return;
        };
        let parent = block::BlockParent::Uuid(self.block.id());
        let picked = pending.picker.handle(ui.ctx(), editors, parent);
        let pick = match picked {
            Some(result) => Some(BlockPick::Chosen {
                block_id: result.id.into_bytes(),
                block_type: result.block_type.into_bytes(),
            }),
            None if pending.picker.is_open() => None,
            None => Some(BlockPick::Cancelled),
        };
        let Some(pick) = pick else {
            return;
        };
        let request_id = pending.request_id;
        self.block_pick = None;
        crate::plugin_host::block_picked(plugin_id, self.instance, request_id, pick);
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

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        if !self.has_region(EditorRegion::Preview) {
            return false;
        }
        self.context = Some(context.painter.ctx().clone());
        crate::plugin_host::preview(
            context.painter,
            crate::plugin_host::PreviewSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                block_id: self.block.id(),
                block_type: self.block.block_type(),
                instance: self.instance,
                corners: context.corners,
                opacity: context.opacity,
            },
        )
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        crate::plugin_host::aspect_ratio(&self.plugin.identity.id, self.instance)
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        self.plugin.capabilities.preserve_aspect_ratio
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

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        if self.presenting() {
            return None;
        }
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Toolbar,
        )
        .map_or_else(|| toolbar_height(ui), |size| size.y.max(1.0));
        let size = egui::vec2(ui.available_width(), height);
        self.region_ui(ui, editors, EditorRegion::Toolbar, size, None)
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.has_region(EditorRegion::LeftSidebar) && !self.presenting()
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::LeftSidebar, size, None)
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.has_region(EditorRegion::RightSidebar) && !self.presenting()
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::RightSidebar, size, None)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
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
        let view = self.plugin.capabilities.pan_and_zoom.then(|| {
            viewport
                .content_rect()
                .unwrap_or(rect)
                .translate(-rect.min.to_vec2())
        });
        let action = self.region_ui(ui, editors, EditorRegion::Main, rect.size(), view);
        self.take_view_changes(rect, viewport);
        action
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

fn toolbar_height(ui: &egui::Ui) -> f32 {
    let spacing = ui.spacing();
    spacing
        .interact_size
        .y
        .max(ui.text_style_height(&egui::TextStyle::Body) + 2.0 * spacing.button_padding.y)
}

pub(super) struct PluginArtifact {
    plugin: Arc<PluginManifest>,
    block: EditorBlock,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    resync: bool,
    outcome: Option<Result<(), String>>,
    regenerating: bool,
}

impl PluginArtifact {
    pub(super) fn new(plugin: Arc<PluginManifest>, target_id: Uuid, target_type: Uuid) -> Self {
        Self {
            plugin,
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
                role: InstanceRole::Artifact(self.block),
                instance: self.instance,
                region: EditorRegion::ArtifactSettings,
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
