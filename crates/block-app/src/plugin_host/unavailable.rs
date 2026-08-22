use block_plugin_api::{EditorInstanceId, EditorRegion};
use eframe::egui;
use uuid::Uuid;

use super::{ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorSlot, PreviewSlot};

pub(crate) fn install(_creation_context: &eframe::CreationContext<'_>) {}

pub(crate) fn editor_ui(ui: &mut egui::Ui, slot: EditorSlot<'_>) -> Option<(Uuid, Uuid)> {
    let EditorSlot {
        plugin,
        block_types,
        client,
        role,
        instance,
        region,
        size,
    } = slot;
    let _ = (block_types, client, instance, region, size);
    let _ = role.block().map(|block| (block.id, block.block_type));
    ui.colored_label(
        egui::Color32::RED,
        format!("{} is not supported on this platform.", plugin.display_name),
    );
    None
}

pub(crate) fn creation(context: &egui::Context, slot: CreationSlot<'_>) -> CreationState {
    let CreationSlot {
        plugin,
        block_types,
        client,
        instance,
    } = slot;
    let _ = (context, block_types, client, instance);
    CreationState::Failed(format!(
        "{} is not supported on this platform.",
        plugin.display_name
    ))
}

pub(crate) fn artifact(context: &egui::Context, slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        plugin,
        block_types,
        client,
        instance,
        block,
        data,
        resync,
    } = slot;
    let _ = (context, block_types, client, instance, block, data, resync);
    ArtifactState::Failed(format!(
        "{} is not supported on this platform.",
        plugin.display_name
    ))
}

pub(crate) fn artifact_draft(_plugin_id: &str, _instance: EditorInstanceId) -> Option<Vec<u8>> {
    None
}

pub(crate) fn regenerate_artifact(_plugin_id: &str, _instance: EditorInstanceId, _data: &[u8]) {}

pub(crate) fn take_artifact_outcome(
    _plugin_id: &str,
    _instance: EditorInstanceId,
) -> Option<Result<(), String>> {
    None
}

pub(crate) fn preview(painter: &egui::Painter, slot: PreviewSlot<'_>) -> bool {
    let PreviewSlot {
        plugin,
        block_types,
        client,
        block_id,
        block_type,
        instance,
        corners,
        opacity,
    } = slot;
    let _ = (
        painter,
        plugin,
        block_types,
        client,
        block_id,
        block_type,
        instance,
        corners,
        opacity,
    );
    false
}

pub(crate) fn creation_ready(_plugin_id: &str, _instance: EditorInstanceId) -> bool {
    false
}

pub(crate) fn commit_creation(_plugin_id: &str, _instance: EditorInstanceId) {}

pub(crate) fn take_created(
    _plugin_id: &str,
    _instance: EditorInstanceId,
) -> Option<Result<Uuid, String>> {
    None
}

pub(crate) fn aspect_ratio(_plugin_id: &str, _instance: EditorInstanceId) -> Option<f32> {
    None
}

pub(crate) fn region_size(
    _plugin_id: &str,
    _instance: EditorInstanceId,
    _region: EditorRegion,
) -> Option<egui::Vec2> {
    None
}

pub(crate) fn intrinsic_size(_plugin_id: &str, _instance: EditorInstanceId) -> Option<egui::Vec2> {
    None
}

pub(crate) fn close(_ctx: &egui::Context, _plugin_id: &str, _instance: EditorInstanceId) {}

pub(crate) fn running() -> Vec<super::RuntimeStatus> {
    Vec::new()
}

pub(crate) fn kill(_ctx: &egui::Context, _plugin_id: &str) {}
