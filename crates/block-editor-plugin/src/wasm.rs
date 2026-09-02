mod host;
mod surface;
mod transport;

pub(crate) use surface::{Surface, FORMAT};
pub(crate) use transport::{initialize_storage, shutdown, start, step};

pub(crate) fn surface_format() -> eframe::egui_wgpu::wgpu::TextureFormat {
    FORMAT
}
