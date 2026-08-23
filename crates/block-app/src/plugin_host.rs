use std::sync::Arc;

use block_client::BlockClient;
use block_plugin_api::{
    BlockTypeDescriptor, EditorInstanceId, EditorRegion, PluginManifest, ScreenId,
};
use eframe::egui;
use uuid::Uuid;

mod input;
mod instances;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod native;
mod presenter;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod process;
mod runtime;
#[cfg(not(any(target_arch = "wasm32", target_os = "windows", target_os = "linux")))]
mod unavailable;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "windows")]
use windows as platform;

pub(crate) use runtime::{
    artifact, artifact_draft, aspect_ratio, close, commit_creation, creation, creation_ready,
    editor_ui, install, intrinsic_size, kill, poll, preview, regenerate_artifact, region_size,
    running, take_artifact_outcome, take_created, take_view_changes,
};

pub(crate) struct RuntimeStatus {
    pub(crate) plugin_id: String,
    pub(crate) state: String,
    pub(crate) surface: SurfaceStatus,
    pub(crate) pass: u64,
    pub(crate) uptime: Option<std::time::Duration>,
    pub(crate) instances: Vec<InstanceStatus>,
}

pub(crate) struct SurfaceStatus {
    pub(crate) index: u32,
    pub(crate) generation: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) placements: usize,
}

pub(crate) struct InstanceStatus {
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: Option<Uuid>,
    pub(crate) role: &'static str,
    pub(crate) opened: bool,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) intrinsic: Option<egui::Vec2>,
    pub(crate) view: Option<egui::Rect>,
    pub(crate) artifact: Option<ArtifactStatus>,
    pub(crate) screens: Vec<ScreenStatus>,
}

pub(crate) struct ArtifactStatus {
    pub(crate) data: usize,
    pub(crate) draft: Option<usize>,
    pub(crate) description: Option<String>,
}

pub(crate) struct ScreenStatus {
    pub(crate) screen: ScreenId,
    pub(crate) region: EditorRegion,
    pub(crate) logical: egui::Vec2,
    pub(crate) pixels: [u32; 2],
    pub(crate) scale_factor: f32,
    pub(crate) used: Option<egui::Vec2>,
    pub(crate) placement: Option<[u32; 4]>,
    pub(crate) drawn: bool,
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
    pub(crate) view: Option<egui::Rect>,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}

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

pub(crate) struct ArtifactSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block: EditorBlock,
    pub(crate) data: &'a [u8],
    pub(crate) resync: bool,
}

pub(crate) enum ArtifactState {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}
