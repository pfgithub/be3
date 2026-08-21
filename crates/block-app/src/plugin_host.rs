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
pub(crate) use native::{close, editor_ui, install, intrinsic_size, region_size};
#[cfg(not(any(target_arch = "wasm32", target_os = "windows")))]
pub(crate) use unavailable::{close, editor_ui, install, intrinsic_size, region_size};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{close, editor_ui, install, intrinsic_size, region_size};

/// One region of one open editor instance, as the host hands it to whichever
/// plugin runtime this platform has.
pub(crate) struct EditorSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client: Arc<BlockClient>,
    pub(crate) block_id: Uuid,
    pub(crate) block_type: Uuid,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
    pub(crate) size: egui::Vec2,
}
