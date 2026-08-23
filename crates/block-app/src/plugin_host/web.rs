use std::{cell::RefCell, collections::HashMap};

use block_plugin_api::{
    ArtifactDescription, EditorInstanceId, EditorMessage, EditorRegion, Message, PluginManifest,
    ScreenLayout,
};
use eframe::egui;
use uuid::Uuid;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::{
    instances::{Instances, NextScreens},
    presenter::{
        PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, Quad, Region,
    },
    preview_size, ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock,
    EditorSlot, InstanceRole, PreviewSlot,
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
    /// The presenter slot the plugin already holds, the lowest free one, or
    /// the one taken back from the runtime that has been idle longest.
    fn surface_for(&mut self, plugin_id: &str, context: &egui::Context) -> Option<u32> {
        if let Some(runtime) = self.runtimes.get(plugin_id) {
            return Some(runtime.surface);
        }
        let free = (0..super::presenter::MAX_SURFACES).find(|surface| {
            !self
                .runtimes
                .values()
                .any(|runtime| runtime.surface == *surface)
        });
        if let Some(surface) = free {
            return Some(surface);
        }
        let idle = self
            .runtimes
            .iter()
            .filter(|(_, runtime)| runtime.instances.is_empty())
            .min_by_key(|(_, runtime)| runtime.pass)
            .map(|(id, _)| id.clone())?;
        self.shutdown(context, &idle)
    }

    fn shutdown(&mut self, context: &egui::Context, plugin_id: &str) -> Option<u32> {
        let mut runtime = self.runtimes.remove(plugin_id)?;
        if let Some(mut adapter) = runtime.adapter.take() {
            adapter.shutdown();
        }
        if !runtime.starting {
            remove_canvas(&runtime.canvas_id);
        }
        context
            .debug_painter()
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
        Some(runtime.surface)
    }
}

struct Runtime {
    surface: u32,
    plugin_id: String,
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
    context: Option<egui::Context>,
    dirty: bool,
    rendered_pass: u64,
    repaint_at: Option<f64>,
}

impl Runtime {
    fn new(plugin: &PluginManifest, surface: u32) -> Self {
        Self {
            surface,
            plugin_id: plugin.identity.id.clone(),
            canvas_id: format!("plugin-canvas-{}", plugin.identity.id),
            url: plugin
                .entry_points
                .web
                .as_deref()
                .and_then(|entry| {
                    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry)
                })
                .unwrap_or_default(),
            open: false,
            starting: false,
            adapter: None,
            error: None,
            status: PresenterStatus::waiting(),
            instances: Instances::default(),
            layout: ScreenLayout::default(),
            sent: Vec::new(),
            pass: 0,
            context: None,
            dirty: false,
            rendered_pass: u64::MAX,
            repaint_at: None,
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let Some(adapter) = &mut self.adapter else {
            return;
        };
        self.dirty = true;
        if let Err(error) = adapter.send(messages) {
            self.error = Some(error);
        }
    }

    fn take_output(&mut self) -> bool {
        let Some(adapter) = &mut self.adapter else {
            return false;
        };
        let client_messages = adapter.take_client_messages();
        let editor_messages = adapter.take_editor_messages();
        let region_sizes = adapter.take_region_sizes();
        let layout = adapter.take_layout();
        let received = !client_messages.is_empty()
            || !editor_messages.is_empty()
            || !region_sizes.is_empty()
            || layout.is_some();
        if let Some(layout) = layout {
            self.layout = layout;
        }
        for message in client_messages {
            self.instances.client_message(message);
        }
        for message in editor_messages {
            self.instances.editor_message(message);
        }
        self.instances.set_region_sizes(region_sizes);
        received
    }
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let render_available = renderer::install(creation_context);
    STATE.with(|state| {
        state.borrow_mut().render_available = render_available;
    });
}

fn open(plugin: &PluginManifest, context: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            return;
        }
        let Some(surface) = state.surface_for(&plugin.identity.id, context) else {
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
                if let Some(context) = &runtime.context {
                    context.request_repaint();
                }
            });
        });
    });
}

pub(crate) fn editor_ui(ui: &mut egui::Ui, slot: EditorSlot<'_>) -> Option<(Uuid, Uuid)> {
    let EditorSlot {
        plugin,
        block_types,
        client,
        role,
        instance,
        region,
        size,
    } = slot;
    open(plugin, ui.ctx());
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            ui.colored_label(
                egui::Color32::RED,
                "wgpu is not available in this build.".to_owned(),
            );
            return None;
        }
        let Some(runtime) = state.runtimes.get_mut(&plugin.identity.id) else {
            ui.colored_label(
                egui::Color32::RED,
                "Too many plugin runtimes are already presenting.",
            );
            return None;
        };
        runtime.context = Some(ui.ctx().clone());
        begin_pass(runtime, ui.ctx(), ui.ctx().cumulative_pass_nr());
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
            return None;
        }
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let pass = runtime.pass;
        let screen = runtime.instances.report(
            instance,
            region,
            ui.ctx(),
            &client,
            role,
            block_types,
            response.rect.size(),
            ui.ctx().pixels_per_point(),
            pass,
        );
        let messages = runtime.instances.input(instance, region, |input| {
            input.update(ui, &response, screen)
        });
        runtime.send(messages);
        let drag = super::input::block_drag(&response);
        let hovering = drag.as_ref().is_some_and(|drag| !drag.dropped);
        let messages = runtime.instances.drag(instance, region, drag);
        runtime.send(messages);
        if hovering && runtime.instances.drag_accepted(instance) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Alias);
        } else if response.hovered() {
            if let Some(cursor) = runtime.instances.cursor(instance, region) {
                ui.ctx().set_cursor_icon(cursor);
            }
        }
        let open_request = runtime.instances.take_open(instance);
        let Some(atlas_region) = Region::of(
            &runtime.layout,
            runtime.surface,
            screen,
            Quad::upright(response.rect),
        ) else {
            return open_request;
        };
        painter.add(present(runtime, response.rect, atlas_region, pass));
        open_request
    })
}

pub(crate) fn creation(context: &egui::Context, slot: CreationSlot<'_>) -> CreationState {
    let CreationSlot {
        plugin,
        block_types,
        client,
        instance,
    } = slot;
    open(plugin, context);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            return CreationState::Failed("wgpu is not available in this build.".to_owned());
        }
        let Some(runtime) = state.runtimes.get_mut(&plugin.identity.id) else {
            return CreationState::Failed(
                "Too many plugin runtimes are already presenting.".to_owned(),
            );
        };
        runtime.context = Some(context.clone());
        begin_pass(runtime, context, context.cumulative_pass_nr());
        if let Some(error) = runtime.error.clone() {
            return CreationState::Failed(error);
        }
        context.request_repaint();
        match runtime
            .instances
            .report_creation(instance, context, &client, block_types)
        {
            true => CreationState::Ready,
            false => CreationState::Starting,
        }
    })
}

pub(crate) fn artifact(context: &egui::Context, slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        plugin,
        block_types,
        client,
        instance,
        block,
        data,
        resync,
    } = slot;
    open(plugin, context);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            return ArtifactState::Failed("wgpu is not available in this build.".to_owned());
        }
        let Some(runtime) = state.runtimes.get_mut(&plugin.identity.id) else {
            return ArtifactState::Failed(
                "Too many plugin runtimes are already presenting.".to_owned(),
            );
        };
        runtime.context = Some(context.clone());
        begin_pass(runtime, context, context.cumulative_pass_nr());
        if let Some(error) = runtime.error.clone() {
            return ArtifactState::Failed(error);
        }
        let messages = runtime.instances.report_artifact(
            instance,
            context,
            &client,
            block_types,
            block,
            data,
            resync,
        );
        runtime.send(messages);
        match runtime.instances.artifact_description(instance) {
            Some(ArtifactDescription::Described { source, summary }) => ArtifactState::Described {
                source: Uuid::from_bytes(source),
                summary,
            },
            Some(ArtifactDescription::Unreadable(error)) => ArtifactState::Failed(error),
            None => {
                context.request_repaint();
                ArtifactState::Starting
            }
        }
    })
}

pub(crate) fn artifact_draft(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec<u8>> {
    STATE.with(|state| {
        state
            .borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .artifact_draft(instance)
    })
}

pub(crate) fn regenerate_artifact(plugin_id: &str, instance: EditorInstanceId, data: &[u8]) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.runtimes.get_mut(plugin_id) else {
            return;
        };
        let messages = runtime.instances.regenerate_artifact(instance, data);
        runtime.send(messages);
    });
}

pub(crate) fn take_artifact_outcome(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<(), String>> {
    STATE.with(|state| {
        state
            .borrow_mut()
            .runtimes
            .get_mut(plugin_id)?
            .instances
            .take_artifact_outcome(instance)
    })
}

pub(crate) fn preview(painter: &egui::Painter, slot: PreviewSlot<'_>) -> bool {
    let PreviewSlot {
        plugin,
        block_types,
        client,
        block_id,
        block_type,
        instance,
        corners,
        opacity,
    } = slot;
    open(plugin, painter.ctx());
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.render_available {
            return false;
        }
        let Some(runtime) = state.runtimes.get_mut(&plugin.identity.id) else {
            return false;
        };
        let context = painter.ctx().clone();
        runtime.context = Some(context.clone());
        begin_pass(runtime, &context, context.cumulative_pass_nr());
        if runtime.error.is_some() {
            return false;
        }
        let rect = egui::Rect::from_points(&corners);
        let scale_factor = context.pixels_per_point();
        let pass = runtime.pass;
        let screen = runtime.instances.report(
            instance,
            EditorRegion::Preview,
            &context,
            &client,
            InstanceRole::Editor(EditorBlock {
                id: block_id,
                block_type,
            }),
            block_types,
            preview_size(rect.size(), scale_factor),
            scale_factor,
            pass,
        );
        let Some(atlas_region) = Region::of(
            &runtime.layout,
            runtime.surface,
            screen,
            Quad {
                rect,
                corners,
                opacity,
            },
        ) else {
            return false;
        };
        painter.add(present(runtime, rect, atlas_region, pass));
        true
    })
}

fn present(
    runtime: &Runtime,
    rect: egui::Rect,
    region: Region,
    pass: u64,
) -> egui::epaint::PaintCallback {
    eframe::egui_wgpu::Callback::new_paint_callback(
        rect,
        PresenterCallback {
            command: PresenterCommand::Present(renderer::WebFrame {
                size: [runtime.layout.width, runtime.layout.height],
                canvas_id: runtime.canvas_id.clone(),
                plugin_id: runtime.plugin_id.clone(),
                pass,
            }),
            status: runtime.status.clone(),
            region,
        },
    )
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

pub(crate) fn creation_ready(plugin_id: &str, instance: EditorInstanceId) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .runtimes
            .get(plugin_id)
            .is_some_and(|runtime| runtime.instances.creation_ready(instance))
    })
}

pub(crate) fn commit_creation(plugin_id: &str, instance: EditorInstanceId) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.runtimes.get_mut(plugin_id) else {
            return;
        };
        let messages = runtime.instances.commit_creation(instance);
        runtime.send(messages);
    });
}

pub(crate) fn take_created(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<Uuid, String>> {
    STATE.with(|state| {
        state
            .borrow_mut()
            .runtimes
            .get_mut(plugin_id)?
            .instances
            .take_created(instance)
    })
}

pub(crate) fn aspect_ratio(plugin_id: &str, instance: EditorInstanceId) -> Option<f32> {
    STATE.with(|state| {
        state
            .borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .aspect_ratio(instance)
    })
}

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<egui::Vec2> {
    STATE.with(|state| {
        state
            .borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .intrinsic_size(instance)
    })
}

pub(crate) fn close(_ctx: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.runtimes.get_mut(plugin_id) else {
            return;
        };
        if !runtime.instances.remove(instance) {
            return;
        }
        runtime.send(vec![Message::Editor(EditorMessage::Close { instance })]);
    });
}

pub(crate) fn running() -> Vec<super::RuntimeStatus> {
    STATE.with(|state| {
        let state = state.borrow();
        let mut running: Vec<_> = state
            .runtimes
            .iter()
            .map(|(plugin_id, runtime)| super::RuntimeStatus {
                plugin_id: plugin_id.clone(),
                surface: runtime.surface,
                state: match (&runtime.error, runtime.adapter.is_some()) {
                    (Some(error), _) => error.clone(),
                    (None, true) => "running".to_owned(),
                    (None, false) => "starting".to_owned(),
                },
                instances: runtime.instances.statuses(),
            })
            .collect();
        running.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        running
    })
}

pub(crate) fn kill(ctx: &egui::Context, plugin_id: &str) {
    STATE.with(|state| {
        state.borrow_mut().shutdown(ctx, plugin_id);
    });
}

fn begin_pass(runtime: &mut Runtime, context: &egui::Context, pass: u64) {
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
    let awaited = messages.iter().any(|message| {
        matches!(
            message,
            Message::Editor(EditorMessage::Open { .. } | EditorMessage::OpenArtifact { .. })
        )
    });
    runtime.send(messages);
    if awaited {
        context.request_repaint();
    }
    pump_client(runtime, context);
}

pub(crate) fn poll(context: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        for runtime in state.runtimes.values_mut() {
            pump_client(runtime, context);
        }
    });
}

fn pump_client(runtime: &mut Runtime, context: &egui::Context) {
    if runtime.adapter.is_none() {
        return;
    }
    let responses = runtime.instances.client_responses();
    let awaited = !responses.is_empty();
    runtime.send(responses);
    if let Some(adapter) = &mut runtime.adapter {
        if let Err(error) = adapter.poll() {
            runtime.error = Some(error);
        }
    }
    let received = runtime.take_output();
    if awaited || received {
        context.request_repaint();
    }
}

pub(super) fn render(plugin_id: &str, pass: u64) -> bool {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(runtime) = state.runtimes.get_mut(plugin_id) else {
            return false;
        };
        if runtime.rendered_pass == pass || runtime.adapter.is_none() {
            return false;
        }
        runtime.rendered_pass = pass;
        let due = match runtime.repaint_at {
            Some(deadline) if deadline <= now() + REPAINT_SLACK_MILLISECONDS => true,
            Some(deadline) => {
                if let Some(context) = &runtime.context {
                    context.request_repaint_after(std::time::Duration::from_secs_f64(
                        (deadline - now()) / 1000.0,
                    ));
                }
                false
            }
            None => false,
        };
        if !runtime.dirty && !due {
            return false;
        }
        runtime.dirty = false;
        runtime.repaint_at = None;
        let repaint = match runtime.adapter.as_mut().unwrap().render() {
            Ok(repaint) => repaint,
            Err(error) => {
                runtime.error = Some(error);
                if let Some(context) = &runtime.context {
                    context.request_repaint();
                }
                return false;
            }
        };
        let received = runtime.take_output();
        let Some(context) = runtime.context.clone() else {
            return true;
        };
        if received {
            context.request_repaint();
        }
        if let Some(delay) = repaint {
            runtime.repaint_at = Some(now() + delay.as_secs_f64() * 1000.0);
            context.request_repaint_after(delay);
        }
        true
    })
}

const REPAINT_SLACK_MILLISECONDS: f64 = 2.0;

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
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
