use std::{cell::RefCell, collections::HashMap};

use block_client::BlockClient;
use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, Message, PluginManifest, ScreenLayout,
};
use eframe::egui;
use std::sync::Arc;
use uuid::Uuid;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::{
    instances::{Instances, NextScreens},
    presenter::{PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, Region},
};
use adapter::WebProtocolAdapter;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    render_available: bool,
    runtimes: HashMap<String, Runtime>,
}

impl State {
    /// The presenter slot the plugin already holds, or the lowest free one.
    fn surface_for(&self, plugin_id: &str) -> Option<u32> {
        if let Some(runtime) = self.runtimes.get(plugin_id) {
            return Some(runtime.surface);
        }
        (0..super::presenter::MAX_SURFACES).find(|surface| {
            !self
                .runtimes
                .values()
                .any(|runtime| runtime.surface == *surface)
        })
    }
}

struct Runtime {
    surface: u32,
    canvas_id: String,
    url: String,
    open: bool,
    starting: bool,
    adapter: Option<WebProtocolAdapter>,
    error: Option<String>,
    status: PresenterStatus,
    instances: Instances,
    layout: ScreenLayout,
    sent: Vec<block_plugin_api::ScreenRequest>,
    pass: u64,
}

impl Runtime {
    fn new(plugin: &PluginManifest, surface: u32) -> Self {
        Self {
            surface,
            canvas_id: format!("plugin-canvas-{}", plugin.identity.id),
            url: plugin.entry_points.web.clone().unwrap_or_default(),
            open: false,
            starting: false,
            adapter: None,
            error: None,
            status: PresenterStatus::waiting(),
            instances: Instances::default(),
            layout: ScreenLayout::default(),
            sent: Vec::new(),
            pass: 0,
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let Some(adapter) = &mut self.adapter else {
            return;
        };
        if let Err(error) = adapter.send(messages) {
            self.error = Some(error);
        }
    }
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let render_available = renderer::install(creation_context);
    STATE.with(|state| {
        state.borrow_mut().render_available = render_available;
    });
}

fn open(plugin: &PluginManifest) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            return;
        }
        let Some(surface) = state.surface_for(&plugin.identity.id) else {
            return;
        };
        let plugin_id = plugin.identity.id.clone();
        let runtime = state
            .runtimes
            .entry(plugin_id.clone())
            .or_insert_with(|| Runtime::new(plugin, surface));
        runtime.open = true;
        if runtime.starting || runtime.adapter.is_some() {
            return;
        }
        runtime.starting = true;
        let canvas_id = runtime.canvas_id.clone();
        let url = runtime.url.clone();
        create_canvas(&canvas_id);
        wasm_bindgen_futures::spawn_local(async move {
            let result = WebProtocolAdapter::start(url, canvas_id.clone()).await;
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let Some(runtime) = state.runtimes.get_mut(&plugin_id) else {
                    if let Ok(mut adapter) = result {
                        adapter.shutdown();
                    }
                    remove_canvas(&canvas_id);
                    return;
                };
                runtime.starting = false;
                match result {
                    Ok(adapter) if runtime.open => runtime.adapter = Some(adapter),
                    Ok(mut adapter) => {
                        adapter.shutdown();
                        remove_canvas(&canvas_id);
                    }
                    Err(error) => {
                        runtime.error = Some(error);
                        remove_canvas(&canvas_id);
                    }
                }
            });
        });
    });
}

pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    plugin: &PluginManifest,
    client: Arc<BlockClient>,
    block_id: Uuid,
    block_type: Uuid,
    instance: EditorInstanceId,
    region: EditorRegion,
    size: egui::Vec2,
) {
    open(plugin);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            ui.colored_label(
                egui::Color32::RED,
                "wgpu is not available in this build.".to_owned(),
            );
            return;
        }
        let Some(runtime) = state.runtimes.get_mut(&plugin.identity.id) else {
            ui.colored_label(
                egui::Color32::RED,
                "Too many plugin runtimes are already presenting.",
            );
            return;
        };
        begin_pass(runtime, ui.ctx().cumulative_pass_nr());
        let presenter_error = match runtime.status.get() {
            PresenterState::Unsupported(error) | PresenterState::Failed(error) => Some(error),
            _ => None,
        };
        let error = runtime.error.clone().or(presenter_error);
        if let Some(error) = &error {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Retry").clicked() {
                runtime.error = None;
                runtime.open = false;
            }
            return;
        }
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let pass = runtime.pass;
        let screen = runtime.instances.report(
            instance,
            region,
            ui.ctx(),
            client,
            block_id,
            block_type,
            response.rect.size(),
            ui.ctx().pixels_per_point(),
            pass,
        );
        let messages = runtime.instances.input(instance, region, |input| {
            input.update(ui, &response, screen)
        });
        runtime.send(messages);
        let Some(atlas_region) = Region::of(&runtime.layout, runtime.surface, screen) else {
            return;
        };
        let atlas_size = [runtime.layout.width, runtime.layout.height];
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            PresenterCallback {
                command: PresenterCommand::Present(renderer::WebFrame {
                    size: atlas_size,
                    canvas_id: runtime.canvas_id.clone(),
                }),
                status: runtime.status.clone(),
                region: atlas_region,
            },
        ));
    });
}

pub(crate) fn region_size(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> Option<egui::Vec2> {
    STATE.with(|state| {
        state
            .borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .region_size(instance, region)
    })
}

pub(crate) fn close(ctx: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.runtimes.get_mut(plugin_id) else {
            return;
        };
        if !runtime.instances.remove(instance) {
            return;
        }
        runtime.send(vec![Message::Editor(EditorMessage::Close { instance })]);
        if !runtime.instances.is_empty() {
            return;
        }
        let Some(mut runtime) = state.runtimes.remove(plugin_id) else {
            return;
        };
        if let Some(mut adapter) = runtime.adapter.take() {
            adapter.shutdown();
        }
        if !runtime.starting {
            remove_canvas(&runtime.canvas_id);
        }
        ctx.debug_painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                PresenterCallback::<renderer::WebFrame> {
                    command: PresenterCommand::Release,
                    status: runtime.status.clone(),
                    region: Region {
                        surface: runtime.surface,
                        ..Region::default()
                    },
                },
            ));
    });
}

fn begin_pass(runtime: &mut Runtime, pass: u64) {
    if runtime.pass == pass {
        return;
    }
    let previous = runtime.pass;
    runtime.pass = pass;
    if runtime.adapter.is_none() {
        return;
    }
    let NextScreens { opened, screens } = runtime.instances.next_screens(previous);
    let mut messages = opened;
    if runtime.sent != screens {
        runtime.sent.clone_from(&screens);
        messages.push(runtime.instances.screen_set(screens));
    }
    messages.extend(runtime.instances.pending());
    runtime.send(messages);
    if let Some(adapter) = &mut runtime.adapter {
        if let Err(error) = adapter.poll() {
            runtime.error = Some(error);
        }
    }
    let client_messages = runtime
        .adapter
        .as_mut()
        .map(WebProtocolAdapter::take_client_messages)
        .unwrap_or_default();
    let region_sizes = runtime
        .adapter
        .as_mut()
        .map(WebProtocolAdapter::take_region_sizes)
        .unwrap_or_default();
    if let Some(layout) = runtime
        .adapter
        .as_mut()
        .and_then(WebProtocolAdapter::take_layout)
    {
        runtime.layout = layout;
    }
    for message in client_messages {
        runtime.instances.client_message(message);
    }
    runtime.instances.set_region_sizes(region_sizes);
}

fn create_canvas(canvas_id: &str) {
    (|| -> Option<()> {
        let document = web_sys::window()?.document()?;
        if document.get_element_by_id(canvas_id).is_some() {
            return Some(());
        }
        let canvas: web_sys::HtmlCanvasElement =
            document.create_element("canvas").ok()?.dyn_into().ok()?;
        canvas.set_id(canvas_id);
        let _ = canvas.style().set_property("left", "-10000px");
        let _ = canvas.style().set_property("position", "fixed");
        let _ = canvas.style().set_property("top", "0");
        let _ = canvas.style().set_property("visibility", "hidden");
        document.body()?.append_child(&canvas).ok()?;
        Some(())
    })();
}

fn remove_canvas(canvas_id: &str) {
    let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(canvas_id))
    else {
        return;
    };
    canvas.remove();
}
