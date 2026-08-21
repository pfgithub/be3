use block::Block;
use block_client::{BlockClient, BlockHandle};
use block_plugin_api::{
    BlockTypeDescriptor, EditorInstanceId, EditorRegion, InteractionMode, PluginManifest,
    ResizeMode,
};
use eframe::egui;
use egui_material_icons::MaterialIcon;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, OnceLock,
};

pub(super) mod checklist;
pub(super) mod counter;
pub(super) mod hotbar;
pub(super) mod workspace_index;

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorInteraction,
    DirectEditorResize, DirectEditorViewport, EditorAccess, EditorAction, PendingCreation,
};

/// A first-party editor package: the block type it edits, its manifest, and
/// the app-side presentation the host keeps ownership of.
pub(super) trait PluginPackage: 'static {
    type Block: Block;

    const ICON: MaterialIcon;

    fn manifest() -> Arc<PluginManifest>;

    /// The block the new-block menu makes for this package. A package whose
    /// manifest fills its block in through the editor's own dialog first has
    /// none to make here.
    fn new_block(client: &BlockClient) -> Option<BlockHandle<Self::Block>>;
}

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

/// Builds a package's manifest once and hands out shared references to it.
pub(super) fn cached_manifest(
    cache: &'static OnceLock<Arc<PluginManifest>>,
    build: impl FnOnce() -> PluginManifest,
) -> Arc<PluginManifest> {
    Arc::clone(cache.get_or_init(|| Arc::new(build())))
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

fn next_instance() -> EditorInstanceId {
    EditorInstanceId(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed))
}

/// The dialog an editor draws itself when its block cannot be made until the
/// user has filled something in. The editor offers the block it would make as
/// it goes, and the host creates it once the dialog is accepted.
pub(super) struct PluginCreation<P: PluginPackage> {
    plugin: Arc<PluginManifest>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    package: std::marker::PhantomData<P>,
}

impl<P: PluginPackage> PluginCreation<P> {
    pub(super) fn new() -> Self {
        Self {
            plugin: P::manifest(),
            instance: next_instance(),
            context: None,
            package: std::marker::PhantomData,
        }
    }
}

impl<P: PluginPackage> Drop for PluginCreation<P> {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl<P: PluginPackage> PendingCreation for PluginCreation<P> {
    fn ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> bool {
        self.context = Some(ui.ctx().clone());
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
                block: None,
                instance: self.instance,
                region: EditorRegion::Main,
                size: egui::vec2(ui.available_width(), height),
            },
        );
        crate::plugin_host::creation_content(&self.plugin.identity.id, self.instance).is_some()
    }

    fn create(&mut self, client: &BlockClient) -> Result<Box<dyn BlockEditor>, String> {
        let content = crate::plugin_host::creation_content(&self.plugin.identity.id, self.instance)
            .ok_or("Fill in the options first")?;
        let block: P::Block = serde_json::from_str(&content).map_err(|error| {
            format!("{} could not be created: {error}", self.plugin.display_name)
        })?;
        Ok(Box::new(PluginEditor::<P>::new(client.create_block(block))))
    }
}

/// How tall an editor's creation dialog is drawn before it has said how much
/// room it wants.
const CREATION_DIALOG_HEIGHT: f32 = 96.0;

pub(super) struct PluginEditor<P: PluginPackage> {
    plugin: Arc<PluginManifest>,
    block: BlockHandle<P::Block>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
}

impl<P: PluginPackage> PluginEditor<P> {
    pub(super) fn new(block: BlockHandle<P::Block>) -> Self {
        Self {
            plugin: P::manifest(),
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
                block: Some(crate::plugin_host::EditorBlock {
                    client: editors.client_handle(),
                    id: self.block.id(),
                    block_type: <P::Block as Block>::TYPE_ID,
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

impl<P: PluginPackage> Drop for PluginEditor<P> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<P: PluginPackage> BlockEditor for PluginEditor<P> {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
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
                block_type: <P::Block as Block>::TYPE_ID,
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
