use block::Block;
use block_client::{blocks::checklist::Checklist, BlockClient, BlockHandle};
use block_plugin_api::{
    ChildOperations, CreationMode, EditorCapabilities, EditorRegion, EntryPoints, InteractionMode,
    PluginIdentity, PluginManifest, ResizeMode, SurfaceMechanism,
};
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
        super::cached_manifest(&MANIFEST, || PluginManifest {
            identity: PluginIdentity {
                id: "be3.checklist".into(),
                name: "Checklist".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            block_type: Checklist::TYPE_ID.into_bytes(),
            display_name: "Checklist".into(),
            icon: "checklist".into(),
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
                web: Some("/checklist.js".into()),
                windows: Some("checklist-host.exe".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
            ],
        })
    }
}
