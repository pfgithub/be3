use std::sync::Arc;

use block_client::BlockClient;
use block_plugin_api::{BlockTypeDescriptor, EditorInstanceId, EditorRegion, PluginManifest};
use eframe::egui;
use uuid::Uuid;

#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod input;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod instances;
#[cfg(target_os = "windows")]
mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod presenter;
#[cfg(target_os = "windows")]
mod process;
#[cfg(not(any(target_arch = "wasm32", target_os = "windows")))]
mod unavailable;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub(crate) use native::{
    artifact, artifact_draft, aspect_ratio, close, commit_creation, creation, creation_ready,
    editor_ui, install, intrinsic_size, kill, preview, regenerate_artifact, region_size, running,
    take_artifact_outcome, take_created,
};
#[cfg(not(any(target_arch = "wasm32", target_os = "windows")))]
pub(crate) use unavailable::{
    artifact, artifact_draft, aspect_ratio, close, commit_creation, creation, creation_ready,
    editor_ui, install, intrinsic_size, kill, preview, regenerate_artifact, region_size, running,
    take_artifact_outcome, take_created,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{
    artifact, artifact_draft, aspect_ratio, close, commit_creation, creation, creation_ready,
    editor_ui, install, intrinsic_size, kill, preview, regenerate_artifact, region_size, running,
    take_artifact_outcome, take_created,
};

pub(crate) struct RuntimeStatus {
    pub(crate) plugin_id: String,
    pub(crate) surface: u32,
    pub(crate) state: String,
    pub(crate) instances: Vec<InstanceStatus>,
}

pub(crate) struct InstanceStatus {
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: Option<Uuid>,
    pub(crate) regions: Vec<EditorRegion>,
}

pub(crate) struct PreviewSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) corners: [egui::Pos2; 4],
    pub(crate) opacity: f32,
}

#[cfg_attr(
    not(any(target_arch = "wasm32", target_os = "windows")),
    allow(dead_code)
)]
pub(crate) fn preview_size(size: egui::Vec2, scale_factor: f32) -> egui::Vec2 {
    const STEP: f32 = 64.0;
    const MAXIMUM: f32 = 2048.0;
    let scale = scale_factor.max(f32::EPSILON);
    let pixels = size * scale;
    egui::vec2(
        (pixels.x / STEP).ceil() * STEP,
        (pixels.y / STEP).ceil() * STEP,
    )
    .clamp(egui::Vec2::splat(STEP), egui::Vec2::splat(MAXIMUM))
        / scale
}

pub(crate) struct CreationSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) instance: EditorInstanceId,
}

pub(crate) enum CreationState {
    Starting,
    Ready,
    Failed(String),
}

pub(crate) struct EditorSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
    pub(crate) size: egui::Vec2,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}

/// What an instance was opened for: a block it edits, a block it is filling
/// in, or a dynamic artifact one of its blocks generated.
#[derive(Clone, Copy)]
pub(crate) enum InstanceRole {
    Editor(EditorBlock),
    Creation,
    Artifact(EditorBlock),
}

impl InstanceRole {
    pub(crate) fn block(self) -> Option<EditorBlock> {
        match self {
            Self::Editor(block) | Self::Artifact(block) => Some(block),
            Self::Creation => None,
        }
    }
}

/// An instance kept open to describe, edit and rebuild one dynamic artifact.
pub(crate) struct ArtifactSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: EditorBlock,
    pub(crate) data: &'a [u8],
    /// Hands the settings over again even when they have not changed, so a
    /// dismissed dialog's edits are thrown away.
    pub(crate) resync: bool,
}

/// What the plugin has said about the settings the host last handed it.
#[cfg_attr(
    not(any(target_arch = "wasm32", target_os = "windows")),
    allow(dead_code)
)]
pub(crate) enum ArtifactState {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}
