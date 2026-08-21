use block::Block;
use block_client::blocks::workspace_index::WorkspaceIndex;
use block_plugin_api::{
    ChildOperations, CreationMode, EditorRegion, EntryPoints, PluginIdentity, PluginManifest,
    ResizeMode, SurfaceMechanism,
};
use egui_material_icons::{icons::ICON_FOLDER, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct WorkspaceIndexPlugin;

impl PluginPackage for WorkspaceIndexPlugin {
    type Block = WorkspaceIndex;

    const ICON: MaterialIcon = ICON_FOLDER;

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(&MANIFEST, || PluginManifest {
            identity: PluginIdentity {
                id: "be3.workspace-index".into(),
                name: "Folder".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            block_type: WorkspaceIndex::TYPE_ID.into_bytes(),
            display_name: "Folder".into(),
            icon: "folder".into(),
            creation: CreationMode::Immediate,
            children: ChildOperations {
                add: true,
                delete: true,
                replace: true,
            },
            important: true,
            resize: ResizeMode::None,
            regions: vec![EditorRegion::Main, EditorRegion::Toolbar],
            entry_points: EntryPoints {
                web: Some("/workspace_index.js".into()),
                windows: Some("workspace_index-host.exe".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
            ],
        })
    }
}
