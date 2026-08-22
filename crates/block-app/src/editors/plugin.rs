use block_client::{blocks, BlockClient, BlockHandleAccess};
use block_plugin_api::{
    BlockTypeDescriptor, CreationMode, EditorInstanceId, EditorRegion, InteractionMode,
    PluginManifest, ResizeMode,
};
use eframe::egui;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use uuid::Uuid;

pub(crate) mod discovery;

use super::{
    BlockEditor, BlockRenderContext, CreationStep, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorResize, DirectEditorViewport, EditorAccess, EditorAction,
    PendingCreation,
};
use crate::plugin_host::{CreationSlot, CreationState};

/// The registered block types as a plugin sees them, built once for every
/// runtime that has to name and illustrate blocks it only holds a reference
/// to.
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
                block: None,
                instance: self.instance,
                region: EditorRegion::Main,
                size: egui::vec2(ui.available_width(), height),
            },
        );
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

const CREATION_DIALOG_HEIGHT: f32 = 96.0;

pub(super) struct PluginEditor {
    plugin: Arc<PluginManifest>,
    block: Box<dyn BlockHandleAccess>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
}

impl PluginEditor {
    pub(super) fn new(plugin: Arc<PluginManifest>, block: Box<dyn BlockHandleAccess>) -> Self {
        Self {
            plugin,
            block,
            instance: next_instance(),
            context: None,
        }
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
    ) -> Option<EditorAction> {
        if !self.has_region(region) {
            return None;
        }
        self.context = Some(ui.ctx().clone());
        let (id, block_type) = crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                block: Some(crate::plugin_host::EditorBlock {
                    id: self.block.id(),
                    block_type: self.block.block_type(),
                }),
                instance: self.instance,
                region,
                size,
            },
        )?;
        Some(EditorAction::OpenBlock { id, block_type })
    }

    fn close(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl Drop for PluginEditor {
    fn drop(&mut self) {
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

    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        self.plugin.capabilities.pan_and_zoom
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
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Toolbar,
        )
        .map_or_else(|| toolbar_height(ui), |size| size.y.max(1.0));
        let size = egui::vec2(ui.available_width(), height);
        self.region_ui(ui, editors, EditorRegion::Toolbar, size)
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.has_region(EditorRegion::LeftSidebar)
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::LeftSidebar, size)
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.has_region(EditorRegion::RightSidebar)
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::RightSidebar, size)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::Main, size)
    }

    fn tab_closed(&mut self) {
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
