use block::Block;
use block_client::blocks::checklist::Checklist;
use block_plugin_api::{
    CreationMode, EditorRegion, EntryPoints, PluginIdentity, PluginManifest, SurfaceMechanism,
};
use egui_material_icons::{icons::ICON_CHECKLIST, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct ChecklistPlugin;

impl PluginPackage for ChecklistPlugin {
    type Block = Checklist;

    const ICON: MaterialIcon = ICON_CHECKLIST;

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
            regions: vec![
                EditorRegion::Main,
                EditorRegion::Toolbar,
                EditorRegion::LeftSidebar,
                EditorRegion::RightSidebar,
            ],
            entry_points: EntryPoints {
                web: Some("/checklist.js".into()),
                windows: Some("checklist-host.exe".into()),
                android: Some("com.be3.block.plugin.ChecklistService".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
                SurfaceMechanism::AndroidHardwareBuffer,
            ],
        })
    }
}
