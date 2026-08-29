use std::sync::Arc;

use block_client::BlockClient;
use block_plugin_api::{
    BlockTypeDescriptor, ChildId, ChildLayer, ChildMode, ChildPart, EditorInstanceId, EditorRegion,
    PluginManifest, ScreenId,
};
use eframe::egui;
use uuid::Uuid;

mod backend;
mod input;
mod instances;
mod presenter;
mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod wasm;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use runtime::{
    artifact, artifact_draft, aspect_ratio, block_picked, close, commit_creation, creation,
    creation_ready, editor_ui, flush, install, intrinsic_size, kill, poll, present, presenting,
    preview, regenerate_artifact, region_shown, region_size, report_children, running,
    take_artifact_outcome, take_block_pick, take_created, take_view_changes,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn cache_in(directory: std::path::PathBuf) {
    wasm::cache_in(directory);
}

pub(crate) const MAX_LIVE_CHILDREN: usize = 16;

pub(crate) struct HostChild {
    pub(crate) child: ChildId,
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) rect: egui::Rect,
    pub(crate) clip: egui::Rect,
    pub(crate) layer: ChildLayer,
    pub(crate) mode: ChildMode,
    pub(crate) part: ChildPart,
}

impl HostChild {
    pub(crate) fn is_below(&self) -> bool {
        matches!(self.layer, ChildLayer::Below)
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(self.mode, ChildMode::Active | ChildMode::Live)
    }

    pub(crate) fn is_preview(&self) -> bool {
        matches!(self.mode, ChildMode::Preview)
    }
}

pub(crate) struct PreviewPresentation {
    pub(crate) drawn: bool,
    pub(crate) size: egui::Vec2,
    pub(crate) children: Vec<HostChild>,
}

pub(crate) struct HostChildStatus {
    pub(crate) child: ChildId,
    pub(crate) available: bool,
    pub(crate) intrinsic: Option<egui::Vec2>,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) has_left_sidebar: bool,
    pub(crate) has_right_sidebar: bool,
    pub(crate) error: Option<String>,
}

pub(crate) struct BlockPickRequest {
    pub(crate) request_id: u64,
    pub(crate) block_types: Vec<Uuid>,
    pub(crate) templates: bool,
}

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
    pub(crate) children: usize,
    pub(crate) child_generation: u64,
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
