use block_editor_plugin::egui;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) struct SceneFrame {
    pub(crate) viewport_size_px: [u32; 2],
    pub(crate) view_projection: [[f32; 4]; 4],
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn paint(painter: &egui::Painter, rect: egui::Rect, frame: SceneFrame) {
    painter.add(
        block_editor_plugin::egui_wgpu::Callback::new_paint_callback(
            rect,
            crate::renderer::Scene3DCallback { frame },
        ),
    );
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn paint(_painter: &egui::Painter, _rect: egui::Rect, _frame: SceneFrame) {}
