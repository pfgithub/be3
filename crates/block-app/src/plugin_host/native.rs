use std::{cell::RefCell, path::PathBuf, sync::Arc};

use block_client::blocks::counter::Counter;
use block_plugin_api::{EditorInstanceId, EditorMessage, Message, ScreenLayout};
use eframe::egui;

use super::{
    instances::{Instances, NextScreens},
    presenter::{PresenterCallback, PresenterCommand, PresenterState, Region},
    process::{Process, SurfaceEvent},
    windows::WindowsFrame,
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    HOST.with(|host| {
        host.borrow_mut().presenter_status = super::windows::install(creation_context);
    });
}

pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    client: Arc<block_client::BlockClient>,
    block: block_client::BlockHandle<Counter>,
    instance: EditorInstanceId,
) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if host.process.is_none() {
            host.process = Some(Process::launch(plugin_path()));
        }
        begin_pass(&mut host, ui.ctx());
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
        if host.presenter_status.is_none() {
            ui.colored_label(
                egui::Color32::RED,
                "Windows plugins require the D3D12 renderer.",
            );
            return;
        }
        if let Some(status) = &host.presenter_status {
            match status.get() {
                PresenterState::Failed(error) | PresenterState::Unsupported(error) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Windows plugin presentation failed: {error}"),
                    );
                }
                PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => {
                }
            }
        }
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let pass = host.pass;
        let screen = host.instances.report(
            instance,
            client,
            block,
            response.rect.size(),
            ui.ctx().pixels_per_point(),
            pass,
        );
        let messages = host
            .instances
            .input(instance, |input| input.update(ui, &response, screen));
        send(&host, messages);
        let Some(status) = host.presenter_status.clone() else {
            return;
        };
        let Some(region) = Region::of(&host.layout, screen) else {
            return;
        };
        let frame = host
            .pending_frame
            .take()
            .unwrap_or(WindowsFrame::Events(Vec::new()));
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            PresenterCallback {
                command: PresenterCommand::Present(frame),
                status,
                region,
            },
        ));
    });
}

pub(crate) fn close(ctx: &egui::Context, instance: EditorInstanceId) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if !host.instances.remove(instance) {
            return;
        }
        send(
            &host,
            vec![Message::Editor(EditorMessage::Close { instance })],
        );
        if !host.instances.is_empty() {
            return;
        }
        if let Some(mut process) = host.process.take() {
            process.shutdown();
        }
        host.layout = ScreenLayout::default();
        host.pending_layouts.clear();
        host.sent.clear();
        host.pending_frame = None;
        if let Some(status) = host.presenter_status.clone() {
            ctx.debug_painter()
                .add(eframe::egui_wgpu::Callback::new_paint_callback(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                    PresenterCallback::<WindowsFrame> {
                        command: PresenterCommand::Release,
                        status,
                        region: Region::default(),
                    },
                ));
        }
    });
}

#[derive(Default)]
struct Host {
    process: Option<Process>,
    presenter_status: Option<super::presenter::PresenterStatus>,
    pending_frame: Option<WindowsFrame>,
    instances: Instances,
    layout: ScreenLayout,
    pending_layouts: Vec<ScreenLayout>,
    sent: Vec<block_plugin_api::ScreenRequest>,
    pass: u64,
}

fn begin_pass(host: &mut Host, ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    if host.pass == pass {
        return;
    }
    let previous = host.pass;
    host.pass = pass;
    let NextScreens { opened, screens } = host.instances.next_screens(previous);
    let mut messages = opened;
    if host.sent != screens {
        host.sent.clone_from(&screens);
        messages.push(host.instances.screen_set(screens));
    }
    messages.extend(host.instances.pending());
    send(host, messages);
    let Some(process) = &host.process else {
        return;
    };
    host.pending_layouts.extend(process.layouts());
    let frames = process.latest_surface();
    for event in &frames {
        let SurfaceEvent::Surface(descriptor, _) = event else {
            continue;
        };
        let Some(index) = host
            .pending_layouts
            .iter()
            .position(|layout| layout.generation == descriptor.generation)
        else {
            continue;
        };
        host.layout = host.pending_layouts.remove(index);
        host.pending_layouts
            .retain(|layout| layout.generation > host.layout.generation);
    }
    if !frames.is_empty() {
        host.pending_frame = Some(WindowsFrame::Events(frames));
    }
    let messages = process.client_messages();
    for message in messages {
        host.instances.client_message(message);
    }
}

fn send(host: &Host, messages: Vec<Message>) {
    if messages.is_empty() {
        return;
    }
    if let Some(process) = &host.process {
        process.send(messages);
    }
}

fn plugin_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.set_file_name("counter-host.exe");
    path
}
