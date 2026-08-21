use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::{
    ChildOperations, CreationMode, EditorCapabilities, EditorRegion, EntryPoints, InteractionMode,
    PluginIdentity, PluginManifest, ResizeMode, SurfaceMechanism,
};
use egui_material_icons::{icons::ICON_123, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct CounterPlugin;

impl PluginPackage for CounterPlugin {
    type Block = Counter;

    const ICON: MaterialIcon = ICON_123;

    fn new_block(client: &BlockClient) -> Option<BlockHandle<Counter>> {
        Some(client.create_block(Counter::default()))
    }

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(&MANIFEST, || PluginManifest {
            identity: PluginIdentity {
                id: "be3.counter".into(),
                name: "Counter".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            block_type: Counter::TYPE_ID.into_bytes(),
            display_name: "Counter".into(),
            icon: "123".into(),
            creation: CreationMode::Immediate,
            children: ChildOperations::default(),
            important: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities::default(),
            resize: ResizeMode::Both,
            regions: vec![
                EditorRegion::Main,
                EditorRegion::Toolbar,
                EditorRegion::LeftSidebar,
                EditorRegion::RightSidebar,
            ],
            entry_points: EntryPoints {
                web: Some("/counter.js".into()),
                windows: Some("counter-host.exe".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
            ],
        })
    }
}
