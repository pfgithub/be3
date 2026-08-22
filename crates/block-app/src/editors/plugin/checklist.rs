use block_plugin_api::PluginManifest;
use std::sync::{Arc, OnceLock};

pub(in crate::editors) fn manifest() -> Arc<PluginManifest> {
    static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
    super::cached_manifest(
        &MANIFEST,
        include_str!("../../../../editors/checklist/manifest.json"),
    )
}
