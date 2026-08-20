use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::{
    CreationMode, EditorInstanceId, EditorRegion, EntryPoints, PluginIdentity, PluginManifest,
    SurfaceMechanism,
};
use eframe::egui;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorInteraction, DirectEditorResize,
    DirectEditorViewport, EditorAccess, EditorAction,
};

pub(super) fn counter_manifest() -> PluginManifest {
    PluginManifest {
        identity: PluginIdentity {
            id: "be3.counter".into(),
            name: "Counter".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        block_type: Counter::TYPE_ID.into_bytes(),
        display_name: "Counter".into(),
        icon: "123".into(),
        creation: CreationMode::Immediate,
        regions: vec![
            EditorRegion::Main,
            EditorRegion::Toolbar,
            EditorRegion::LeftSidebar,
            EditorRegion::RightSidebar,
        ],
        entry_points: EntryPoints {
            web: Some("/counter.js".into()),
            windows: Some("counter-host.exe".into()),
            android: Some("com.be3.block.plugin.CounterService".into()),
        },
        surfaces: vec![
            SurfaceMechanism::WebExternalImage,
            SurfaceMechanism::WindowsDxgi,
            SurfaceMechanism::AndroidHardwareBuffer,
        ],
    }
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct PluginEditor {
    block: BlockHandle<Counter>,
    instance: EditorInstanceId,
    regions: Vec<EditorRegion>,
    context: Option<egui::Context>,
}

impl PluginEditor {
    pub(super) fn new(_client: &BlockClient, block: BlockHandle<Counter>) -> Self {
        Self {
            block,
            instance: EditorInstanceId(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed)),
            regions: counter_manifest().regions,
            context: None,
        }
    }

    fn has_region(&self, region: EditorRegion) -> bool {
        self.regions.contains(&region)
    }

    fn region_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        region: EditorRegion,
        size: egui::Vec2,
    ) {
        if !self.has_region(region) {
            return;
        }
        self.context = Some(ui.ctx().clone());
        crate::plugin_host::editor_ui(
            ui,
            editors.client_handle(),
            self.block.clone(),
            self.instance,
            region,
            size,
        );
    }

    fn close(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, self.instance);
        }
    }
}

impl Drop for PluginEditor {
    fn drop(&mut self) {
        self.close();
    }
}

impl BlockEditor for PluginEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        DirectEditorInteraction::Live
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        DirectEditorResize::Both
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        Some(egui::vec2(420.0, 240.0))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let size = egui::vec2(ui.available_width(), toolbar_height(ui));
        self.region_ui(ui, editors, EditorRegion::Toolbar, size);
        None
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
        self.region_ui(ui, editors, EditorRegion::LeftSidebar, size);
        None
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
        self.region_ui(ui, editors, EditorRegion::RightSidebar, size);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let size = ui.available_size();
        self.region_ui(ui, editors, EditorRegion::Main, size);
        None
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
