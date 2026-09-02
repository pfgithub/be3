use block_editor_plugin::egui;

use crate::frame::RenderFrame;

#[cfg(target_arch = "wasm32")]
pub(crate) fn paint(painter: &egui::Painter, rect: egui::Rect, frame: RenderFrame) {
    painter.add(
        block_editor_plugin::egui_wgpu::Callback::new_paint_callback(
            rect,
            crate::renderer::GridCallback { frame },
        ),
    );
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn paint(_painter: &egui::Painter, _rect: egui::Rect, _frame: RenderFrame) {}
