use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use block_plugin_api::{
    ArtifactDescription, EditorInstanceId, EditorRegion, Message, PluginManifest, ScreenLayout,
};
use eframe::egui;
use uuid::Uuid;

#[cfg(target_os = "linux")]
use super::linux::{install as install_presenter, LinuxFrame as PlatformFrame, RENDERER_REQUIRED};
#[cfg(target_os = "windows")]
use super::windows::{
    install as install_presenter, WindowsFrame as PlatformFrame, RENDERER_REQUIRED,
};
use super::{
    core::{select_surface, RuntimeCore, SurfaceSelection},
    presenter::{PresenterCallback, PresenterCommand, PresenterState, Quad, Region},
    preview_size,
    process::{Process, SurfaceEvent},
    ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, EditorSlot,
    InstanceRole, PreviewSlot,
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    HOST.with(|host| {
        host.borrow_mut().presenter_available = install_presenter(creation_context);
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
        view,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.presenter_available {
            ui.colored_label(egui::Color32::RED, RENDERER_REQUIRED);
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
        detect_exit(runtime);
        if let Some(error) = &runtime.exit {
            ui.colored_label(
                egui::Color32::RED,
                format!("plugin process exited: {error}"),
            );
            if ui.button("Restart plugin").clicked() {
                runtime.exit = None;
                runtime.process = Some(Process::launch(plugin_path(plugin), ui.ctx().clone()));
            }
            return None;
        } else if runtime.process.is_none() {
            runtime.process = Some(Process::launch(plugin_path(plugin), ui.ctx().clone()));
        }
        begin_pass(runtime, pass);
        match runtime.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("plugin presentation failed: {error}"),
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
        if let Some(view) = view {
            runtime.instances.set_view(instance, view);
        }
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
            return CreationState::Failed(RENDERER_REQUIRED.to_owned());
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
        detect_exit(runtime);
        if let Some(error) = &runtime.exit {
            return CreationState::Failed(format!("plugin process exited: {error}"));
        } else if runtime.process.is_none() {
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
            return ArtifactState::Failed(RENDERER_REQUIRED.to_owned());
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
        detect_exit(runtime);
        if let Some(error) = &runtime.exit {
            return ArtifactState::Failed(format!("plugin process exited: {error}"));
        } else if runtime.process.is_none() {
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
        detect_exit(runtime);
        if runtime.exit.is_some() {
            return false;
        } else if runtime.process.is_none() {
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
        .unwrap_or(PlatformFrame::Events(Vec::new()));
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
            .region_size(instance, region)
    })
}

pub(crate) fn creation_ready(plugin_id: &str, instance: EditorInstanceId) -> bool {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)
            .is_some_and(|runtime| runtime.creation_ready(instance))
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

pub(crate) fn take_view_changes(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<block_plugin_api::ViewChange> {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .get_mut(plugin_id)
            .map(|runtime| runtime.instances.take_view_changes(instance))
            .unwrap_or_default()
    })
}

pub(crate) fn aspect_ratio(plugin_id: &str, instance: EditorInstanceId) -> Option<f32> {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .aspect_ratio(instance)
    })
}

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<egui::Vec2> {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .get(plugin_id)?
            .intrinsic_size(instance)
    })
}

pub(crate) fn close(_ctx: &egui::Context, plugin_id: &str, instance: EditorInstanceId) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(runtime) = host.runtimes.get_mut(plugin_id) else {
            return;
        };
        if let Some(messages) = runtime.close(instance) {
            runtime.send(messages);
        }
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
                state: match &runtime.exit {
                    Some(error) => format!("exited: {error}"),
                    None => match runtime.status.get() {
                        PresenterState::Waiting => "waiting".to_owned(),
                        PresenterState::Presenting => "presenting".to_owned(),
                        PresenterState::Released => "released".to_owned(),
                        PresenterState::Failed(error) | PresenterState::Unsupported(error) => error,
                    },
                },
                instances: runtime.instances.statuses(),
            })
            .collect();
        running.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        running
    })
}

pub(crate) fn poll(_context: &egui::Context) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        for runtime in host.runtimes.values_mut() {
            detect_exit(runtime);
            pump_client(runtime);
        }
    });
}

fn pump_client(runtime: &mut Runtime) {
    let Some(process) = &runtime.process else {
        return;
    };
    let requests = process.client_messages();
    for message in requests {
        runtime.instances.client_message(message);
    }
    let responses = runtime.instances.client_responses();
    runtime.send(responses);
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
    fn surface_for(&mut self, plugin_id: &str, ctx: &egui::Context) -> Option<u32> {
        match select_surface(
            plugin_id,
            self.runtimes
                .iter()
                .map(|(id, runtime)| (id, &runtime.core)),
        )? {
            SurfaceSelection::Selected(surface) => Some(surface),
            SurfaceSelection::Evict(id) => self.shutdown(ctx, &id),
        }
    }

    fn shutdown(&mut self, ctx: &egui::Context, plugin_id: &str) -> Option<u32> {
        let mut runtime = self.runtimes.remove(plugin_id)?;
        if let Some(mut process) = runtime.process.take() {
            process.shutdown();
        }
        ctx.debug_painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                PresenterCallback::<PlatformFrame> {
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
    core: RuntimeCore,
    process: Option<Process>,
    exit: Option<String>,
    pending_frame: Option<PlatformFrame>,
    pending_layouts: Vec<ScreenLayout>,
}

impl Runtime {
    fn new(surface: u32) -> Self {
        Self {
            core: RuntimeCore::new(surface),
            process: None,
            exit: None,
            pending_frame: None,
            pending_layouts: Vec::new(),
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

impl std::ops::Deref for Runtime {
    type Target = RuntimeCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl std::ops::DerefMut for Runtime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

fn detect_exit(runtime: &mut Runtime) {
    let Some(error) = runtime.process.as_ref().and_then(Process::take_exit) else {
        return;
    };
    runtime.process = None;
    runtime.exit = Some(error);
}

fn begin_pass(runtime: &mut Runtime, pass: u64) {
    let Some((messages, _)) = runtime.core.begin_pass(pass) else {
        return;
    };
    runtime.send(messages);
    let (layouts, frames, editor_messages, sizes) = {
        let Some(process) = &runtime.process else {
            return;
        };
        (
            process.layouts(),
            process.latest_surface(),
            process.editor_messages(),
            process.region_sizes(),
        )
    };
    runtime.pending_layouts.extend(layouts);
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
        runtime.core.layout = runtime.pending_layouts.remove(index);
        let generation = runtime.core.layout.generation;
        runtime
            .pending_layouts
            .retain(|layout| layout.generation > generation);
    }
    if !frames.is_empty() {
        runtime.pending_frame = Some(PlatformFrame::Events(frames));
    }
    for message in editor_messages {
        runtime.instances.editor_message(message);
    }
    runtime.instances.set_region_sizes(sizes);
    pump_client(runtime);
}

fn plugin_path(plugin: &PluginManifest) -> PathBuf {
    #[cfg(target_os = "windows")]
    let entry = plugin.entry_points.windows.as_deref().unwrap_or_default();
    #[cfg(target_os = "linux")]
    let entry = plugin.entry_points.linux.as_deref().unwrap_or_default();
    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry).unwrap_or_default()
}
