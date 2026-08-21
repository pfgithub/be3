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
    aspect_ratio, close, commit_creation, creation_ready, editor_ui, install, intrinsic_size,
    preview, region_size, take_created,
};
#[cfg(not(any(target_arch = "wasm32", target_os = "windows")))]
pub(crate) use unavailable::{
    aspect_ratio, close, commit_creation, creation_ready, editor_ui, install, intrinsic_size,
    preview, region_size, take_created,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{
    aspect_ratio, close, commit_creation, creation_ready, editor_ui, install, intrinsic_size,
    preview, region_size, take_created,
};

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

pub(crate) struct EditorSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) block: Option<EditorBlock>,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
    pub(crate) size: egui::Vec2,
}

#[derive(Clone, Copy)]
pub(crate) struct EditorBlock {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}
