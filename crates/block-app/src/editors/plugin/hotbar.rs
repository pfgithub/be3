use block_client::{blocks::hotbar::Hotbar, BlockClient, BlockHandle};
use block_plugin_api::PluginManifest;
use egui_material_icons::{icons::ICON_WIDGETS, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct HotbarPlugin;

impl PluginPackage for HotbarPlugin {
    type Block = Hotbar;

    const ICON: MaterialIcon = ICON_WIDGETS;

    fn new_block(_client: &BlockClient) -> Option<BlockHandle<Hotbar>> {
        None
    }

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(
            &MANIFEST,
            include_str!("../../../../editors/hotbar/manifest.json"),
        )
    }
}
