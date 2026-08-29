use std::cell::RefCell;

use block_plugin_api::PreviewLayout;
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};

use super::presenter::Presenter;

thread_local! {
    static RENDER_STATE: RefCell<Option<egui_wgpu::RenderState>> = const { RefCell::new(None) };
}

pub(super) fn install(creation_context: &eframe::CreationContext<'_>) {
    RENDER_STATE.with(|state| {
        state
            .borrow_mut()
            .clone_from(&creation_context.wgpu_render_state);
    });
}

pub(super) fn layer(plugin_id: &str) -> egui::LayerId {
    egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new(("plugin-previews", plugin_id)),
    )
}

pub(super) fn painter(context: &egui::Context, plugin_id: &str, rect: egui::Rect) -> egui::Painter {
    egui::Painter::new(context.clone(), layer(plugin_id), rect)
}

pub(super) fn render(
    context: &egui::Context,
    plugin_id: &str,
    surface: u32,
    layout: &PreviewLayout,
) -> bool {
    let shapes = take_shapes(context, plugin_id);
    if shapes.is_empty() || layout.is_empty() {
        return false;
    }
    RENDER_STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return false;
        };
        let mut renderer = state.renderer.write();
        let Some((view, size)) = renderer
            .callback_resources
            .get::<Presenter>()
            .and_then(|presenter| presenter.preview_texture(surface))
            .filter(|texture| texture.format() == state.target_format)
            .map(|texture| {
                (
                    texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    [texture.width(), texture.height()],
                )
            })
        else {
            return false;
        };
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: layout.scale_factor(),
        };
        let jobs = context.tessellate(shapes, screen.pixels_per_point);
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("plugin previews"),
            });
        let commands =
            renderer.update_buffers(&state.device, &state.queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plugin previews"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        state
            .queue
            .submit(commands.into_iter().chain([encoder.finish()]));
        true
    })
}

fn take_shapes(context: &egui::Context, plugin_id: &str) -> Vec<egui::epaint::ClippedShape> {
    let layer = layer(plugin_id);
    context.graphics_mut(|graphics| {
        graphics.get_mut(layer).map_or_else(Vec::new, |list| {
            std::mem::take(list).all_entries().cloned().collect()
        })
    })
}
