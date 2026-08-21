use block::Block;
use block_client::blocks::hotbar::Hotbar;
use block_plugin_api::{
    ChildOperations, CreationMode, EditorRegion, EntryPoints, InteractionMode, PluginIdentity,
    PluginManifest, ResizeMode, SurfaceMechanism,
};
use egui_material_icons::{icons::ICON_WIDGETS, MaterialIcon};
use std::sync::{Arc, OnceLock};

use super::PluginPackage;

pub(in crate::editors) struct HotbarPlugin;

impl PluginPackage for HotbarPlugin {
    type Block = Hotbar;

    const ICON: MaterialIcon = ICON_WIDGETS;

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(&MANIFEST, || PluginManifest {
            identity: PluginIdentity {
                id: "be3.hotbar".into(),
                name: "Hotbar".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            block_type: Hotbar::TYPE_ID.into_bytes(),
            display_name: "Hotbar".into(),
            icon: "widgets".into(),
            creation: CreationMode::None,
            children: ChildOperations::default(),
            important: false,
            interaction: InteractionMode::Live,
            resize: ResizeMode::Both,
            regions: vec![EditorRegion::Main],
            entry_points: EntryPoints {
                web: Some("/hotbar.js".into()),
                windows: Some("hotbar-host.exe".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
            ],
        })
    }
}
