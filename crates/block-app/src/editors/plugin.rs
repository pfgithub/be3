use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::{
    CreationMode, EditorRegion, EntryPoints, PluginIdentity, PluginManifest, SurfaceMechanism,
};
use eframe::egui;

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
        regions: vec![EditorRegion {
            id: "main".into(),
            main: true,
        }],
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

pub(super) struct PluginEditor {
    block: BlockHandle<Counter>,
    context: Option<egui::Context>,
}

impl PluginEditor {
    pub(super) fn new(_client: &BlockClient, block: BlockHandle<Counter>) -> Self {
        Self {
            block,
            context: None,
        }
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

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.context = Some(ui.ctx().clone());
        crate::plugin_host::editor_ui(ui, _editors.client_handle(), self.block.clone());
        None
    }

    fn tab_closed(&mut self) {
        if let Some(context) = &self.context {
            crate::plugin_host::close(context);
        }
    }
}
