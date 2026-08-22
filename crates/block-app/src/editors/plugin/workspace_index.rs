use block_client::{blocks::workspace_index::WorkspaceIndex, BlockClient, BlockHandle};
use block_plugin_api::PluginManifest;
use egui_material_icons::{icons::ICON_FOLDER, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct WorkspaceIndexPlugin;

impl PluginPackage for WorkspaceIndexPlugin {
    type Block = WorkspaceIndex;

    const ICON: MaterialIcon = ICON_FOLDER;

    fn new_block(client: &BlockClient) -> Option<BlockHandle<WorkspaceIndex>> {
        Some(client.create_block(WorkspaceIndex::default()))
    }

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(
            &MANIFEST,
            include_str!("../../../../editors/workspace_index/manifest.json"),
        )
    }
}
