use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use block_plugin_api::{
    ArtifactDescription, EditorInstanceId, EditorMessage, EditorRegion, Message, PluginManifest,
    ScreenLayout,
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
    ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, EditorSlot,
    InstanceRole, PreviewSlot,
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
        role,
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
        let Some(surface) = host.surface_for(&plugin.identity.id, ui.ctx()) else {
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
        painter.add(present(runtime, response.rect, atlas_region));
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
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            return CreationState::Failed("Windows plugins require the D3D12 renderer.".to_owned());
        }
        let Some(surface) = host.surface_for(&plugin.identity.id, context) else {
            return CreationState::Failed(
                "Too many plugin runtimes are already presenting.".to_owned(),
            );
        };
        let pass = context.cumulative_pass_nr();
        let runtime = host
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(surface));
        if runtime.process.is_none() {
            runtime.process = Some(Process::launch(plugin_path(plugin), context.clone()));
        }
        begin_pass(runtime, pass);
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
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            return ArtifactState::Failed("Windows plugins require the D3D12 renderer.".to_owned());
        }
        let Some(surface) = host.surface_for(&plugin.identity.id, context) else {
            return ArtifactState::Failed(
                "Too many plugin runtimes are already presenting.".to_owned(),
            );
        };
        let pass = context.cumulative_pass_nr();
        let runtime = host
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(surface));
        if runtime.process.is_none() {
            runtime.process = Some(Process::launch(plugin_path(plugin), context.clone()));
        }
        begin_pass(runtime, pass);
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
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .instances
            .artifact_draft(instance)
    })
}

pub(crate) fn regenerate_artifact(plugin_id: &str, instance: EditorInstanceId, data: &[u8]) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(runtime) = host.runtimes.get_mut(plugin_id) else {
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
    HOST.with(|host| {
        host.borrow_mut()
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
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            return false;
        }
        let context = painter.ctx().clone();
        let Some(surface) = host.surface_for(&plugin.identity.id, &context) else {
            return false;
        };
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

pub(crate) fn close(_ctx: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(runtime) = host.runtimes.get_mut(plugin_id) else {
            return;
        };
        if !runtime.instances.remove(instance) {
            return;
        }
        runtime.send(vec![Message::Editor(EditorMessage::Close { instance })]);
    });
}

pub(crate) fn running() -> Vec<super::RuntimeStatus> {
    HOST.with(|host| {
        let host = host.borrow();
        let mut running: Vec<_> = host
            .runtimes
            .iter()
            .map(|(plugin_id, runtime)| super::RuntimeStatus {
                plugin_id: plugin_id.clone(),
                surface: runtime.surface,
                state: match runtime.status.get() {
                    PresenterState::Waiting => "waiting".to_owned(),
                    PresenterState::Presenting => "presenting".to_owned(),
                    PresenterState::Released => "released".to_owned(),
                    PresenterState::Failed(error) | PresenterState::Unsupported(error) => error,
                },
                instances: runtime.instances.statuses(),
            })
            .collect();
        running.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        running
    })
}

pub(crate) fn kill(ctx: &egui::Context, plugin_id: &str) {
    HOST.with(|host| {
        host.borrow_mut().shutdown(ctx, plugin_id);
    });
}

#[derive(Default)]
struct Host {
    presenter_available: bool,
    runtimes: HashMap<String, Runtime>,
}

impl Host {
    /// The presenter slot the plugin already holds, the lowest free one, or
    /// the one taken back from the runtime that has been idle longest.
    fn surface_for(&mut self, plugin_id: &str, ctx: &egui::Context) -> Option<u32> {
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
        self.shutdown(ctx, &idle)
    }

    fn shutdown(&mut self, ctx: &egui::Context, plugin_id: &str) -> Option<u32> {
        let mut runtime = self.runtimes.remove(plugin_id)?;
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
        Some(runtime.surface)
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
    let entry = plugin.entry_points.windows.as_deref().unwrap_or_default();
    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry).unwrap_or_default()
}
