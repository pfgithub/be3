use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, Message, PluginManifest, ScreenLayout,
};
use eframe::egui;
use uuid::Uuid;

use super::{
    instances::{Instances, NextScreens},
    presenter::{
        PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, Quad, Region,
    },
    preview_size,
    process::{Process, SurfaceEvent},
    windows::WindowsFrame,
    EditorBlock, EditorSlot, PreviewSlot,
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    HOST.with(|host| {
        host.borrow_mut().presenter_available = super::windows::install(creation_context);
    });
}

pub(crate) fn editor_ui(ui: &mut egui::Ui, slot: EditorSlot<'_>) -> Option<(Uuid, Uuid)> {
    let EditorSlot {
        plugin,
        block_types,
        client,
        block,
        instance,
        region,
        size,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            ui.colored_label(
                egui::Color32::RED,
                "Windows plugins require the D3D12 renderer.",
            );
            return None;
        }
        let Some(surface) = host.surface_for(&plugin.identity.id) else {
            ui.colored_label(
                egui::Color32::RED,
                "Too many plugin runtimes are already presenting.",
            );
            return None;
        };
        let pass = ui.ctx().cumulative_pass_nr();
        let runtime = host
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(surface));
        if runtime.process.is_none() {
            runtime.process = Some(Process::launch(plugin_path(plugin), ui.ctx().clone()));
        }
        begin_pass(runtime, pass);
        match runtime.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("Windows plugin presentation failed: {error}"),
                );
            }
            PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => {}
        }
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let screen = runtime.instances.report(
            instance,
            region,
            ui.ctx(),
            &client,
            block,
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
        painter.add(present(runtime, response.rect, atlas_region));
        open_request
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
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            return false;
        }
        let Some(surface) = host.surface_for(&plugin.identity.id) else {
            return false;
        };
        let context = painter.ctx().clone();
        let pass = context.cumulative_pass_nr();
        let runtime = host
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(surface));
        if runtime.process.is_none() {
            runtime.process = Some(Process::launch(plugin_path(plugin), context.clone()));
        }
        begin_pass(runtime, pass);
        let rect = egui::Rect::from_points(&corners);
        let scale_factor = context.pixels_per_point();
        let screen = runtime.instances.report(
            instance,
            EditorRegion::Preview,
            &context,
            &client,
            Some(EditorBlock {
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
        painter.add(present(runtime, rect, atlas_region));
        true
    })
}

fn present(runtime: &mut Runtime, rect: egui::Rect, region: Region) -> egui::epaint::PaintCallback {
    let frame = runtime
        .pending_frame
        .take()
        .unwrap_or(WindowsFrame::Events(Vec::new()));
    eframe::egui_wgpu::Callback::new_paint_callback(
        rect,
        PresenterCallback {
            command: PresenterCommand::Present(frame),
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
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .region_size(instance, region)
    })
}

pub(crate) fn creation_ready(plugin_id: &str, instance: EditorInstanceId) -> bool {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)
            .is_some_and(|runtime| runtime.instances.creation_ready(instance))
    })
}

pub(crate) fn commit_creation(plugin_id: &str, instance: EditorInstanceId) {
    HOST.with(|host| {
        let host = host.borrow();
        let Some(runtime) = host.runtimes.get(plugin_id) else {
            return;
        };
        runtime.send(runtime.instances.commit_creation(instance));
    });
}

pub(crate) fn take_created(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<Uuid, String>> {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .get_mut(plugin_id)?
            .instances
            .take_created(instance)
    })
}

pub(crate) fn aspect_ratio(plugin_id: &str, instance: EditorInstanceId) -> Option<f32> {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .aspect_ratio(instance)
    })
}

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<egui::Vec2> {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .intrinsic_size(instance)
    })
}

pub(crate) fn close(ctx: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(runtime) = host.runtimes.get_mut(plugin_id) else {
            return;
        };
        if !runtime.instances.remove(instance) {
            return;
        }
        runtime.send(vec![Message::Editor(EditorMessage::Close { instance })]);
        if !runtime.instances.is_empty() {
            return;
        }
        let Some(mut runtime) = host.runtimes.remove(plugin_id) else {
            return;
        };
        if let Some(mut process) = runtime.process.take() {
            process.shutdown();
        }
        ctx.debug_painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                PresenterCallback::<WindowsFrame> {
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

#[derive(Default)]
struct Host {
    presenter_available: bool,
    runtimes: HashMap<String, Runtime>,
}

impl Host {
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
    status: PresenterStatus,
    process: Option<Process>,
    pending_frame: Option<WindowsFrame>,
    instances: Instances,
    layout: ScreenLayout,
    pending_layouts: Vec<ScreenLayout>,
    sent: Vec<block_plugin_api::ScreenRequest>,
    pass: u64,
}

impl Runtime {
    fn new(surface: u32) -> Self {
        Self {
            surface,
            status: PresenterStatus::waiting(),
            process: None,
            pending_frame: None,
            instances: Instances::default(),
            layout: ScreenLayout::default(),
            pending_layouts: Vec::new(),
            sent: Vec::new(),
            pass: 0,
        }
    }

    fn send(&self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        if let Some(process) = &self.process {
            process.send(messages);
        }
    }
}

fn begin_pass(runtime: &mut Runtime, pass: u64) {
    if runtime.pass == pass {
        return;
    }
    let previous = runtime.pass;
    runtime.pass = pass;
    let NextScreens { opened, screens } = runtime.instances.next_screens(previous);
    let mut messages = opened;
    if runtime.sent != screens {
        runtime.sent.clone_from(&screens);
        messages.push(runtime.instances.screen_set(screens));
    }
    messages.extend(runtime.instances.pending());
    runtime.send(messages);
    let Some(process) = &runtime.process else {
        return;
    };
    runtime.pending_layouts.extend(process.layouts());
    let frames = process.latest_surface();
    for event in &frames {
        let SurfaceEvent::Surface(descriptor, _) = event else {
            continue;
        };
        let Some(index) = runtime
            .pending_layouts
            .iter()
            .position(|layout| layout.generation == descriptor.generation)
        else {
            continue;
        };
        runtime.layout = runtime.pending_layouts.remove(index);
        runtime
            .pending_layouts
            .retain(|layout| layout.generation > runtime.layout.generation);
    }
    if !frames.is_empty() {
        runtime.pending_frame = Some(WindowsFrame::Events(frames));
    }
    let messages = process.client_messages();
    let editor_messages = process.editor_messages();
    let sizes = process.region_sizes();
    for message in messages {
        runtime.instances.client_message(message);
    }
    for message in editor_messages {
        runtime.instances.editor_message(message);
    }
    runtime.instances.set_region_sizes(sizes);
}

fn plugin_path(plugin: &PluginManifest) -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.set_file_name(plugin.entry_points.windows.as_deref().unwrap_or_default());
    path
}
