use block_client::{blocks::checklist::Checklist, BlockClient, BlockHandle};
use block_plugin_api::PluginManifest;
use egui_material_icons::{icons::ICON_CHECKLIST, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct ChecklistPlugin;

impl PluginPackage for ChecklistPlugin {
    type Block = Checklist;

    const ICON: MaterialIcon = ICON_CHECKLIST;

    fn new_block(client: &BlockClient) -> Option<BlockHandle<Checklist>> {
        Some(client.create_block(Checklist::default()))
    }

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(
            &MANIFEST,
            include_str!("../../../../editors/checklist/manifest.json"),
        )
    }
}
