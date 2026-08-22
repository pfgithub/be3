use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::PluginManifest;
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
        super::cached_manifest(
            &MANIFEST,
            include_str!("../../../../editors/counter/manifest.json"),
        )
    }
}
