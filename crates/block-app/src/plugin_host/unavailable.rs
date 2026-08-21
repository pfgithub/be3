use std::sync::Arc;

use block_client::BlockClient;
use block_plugin_api::{EditorInstanceId, EditorRegion, PluginManifest};
use eframe::egui;
use uuid::Uuid;

pub(crate) fn install(_creation_context: &eframe::CreationContext<'_>) {}

pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    plugin: &PluginManifest,
    _client: Arc<BlockClient>,
    _block_id: Uuid,
    _block_type: Uuid,
    _instance: EditorInstanceId,
    _region: EditorRegion,
    _size: egui::Vec2,
) -> Option<(Uuid, Uuid)> {
    ui.colored_label(
        egui::Color32::RED,
        format!("{} is not supported on this platform.", plugin.display_name),
    );
    None
}

pub(crate) fn region_size(
    _plugin_id: &str,
    _instance: EditorInstanceId,
    _region: EditorRegion,
) -> Option<egui::Vec2> {
    None
}

pub(crate) fn close(_ctx: &egui::Context, _plugin_id: &str, _instance: EditorInstanceId) {}
