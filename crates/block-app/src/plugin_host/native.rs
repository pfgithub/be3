use std::{cell::RefCell, path::PathBuf, sync::Arc};

use block::Block;
use block_client::blocks::counter::{Counter, CounterOperation};
use block_plugin_api::{
    DelegatedClientMessage, EditorInstanceId, EditorMessage, Message, ViewportMetrics,
};
use eframe::egui;

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
) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.client.get_or_insert(client);
        host.block.get_or_insert(block);
        if host.process.is_none() {
            host.process = Some(super::process::Process::launch(plugin_path()));
        }
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
            use super::presenter::PresenterState;
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
        let messages = host
            .input
            .update(ui, &response, ui.ctx().pixels_per_point());
        if let Some(process) = &host.process {
            process.send(messages);
            let frames = process.latest_surface();
            if !frames.is_empty() {
                host.pending_frame = Some(super::windows::WindowsFrame::Events(frames));
            }
        }
        if !host.opened {
            let client = Arc::clone(host.client.as_ref().unwrap());
            let block = host.block.as_ref().unwrap().clone();
            let size = response.rect.size();
            if let Some(process) = &host.process {
                process.send(vec![Message::Editor(EditorMessage::Open {
                    instance: EditorInstanceId(1),
                    block_id: block.id().into_bytes(),
                    block_type: Counter::TYPE_ID.into_bytes(),
                    account_id: client.account_id().into_bytes(),
                    workspace_id: client.workspace_id().into_bytes(),
                    editable: client.block_access(block.id()) == block::BlockAccess::Edit,
                    metrics: ViewportMetrics {
                        logical_width: size.x,
                        logical_height: size.y,
                        pixel_width: (size.x * ui.ctx().pixels_per_point()).round() as u32,
                        pixel_height: (size.y * ui.ctx().pixels_per_point()).round() as u32,
                        scale_factor: ui.ctx().pixels_per_point(),
                    },
                })]);
                host.opened = true;
            }
        }
        let client_messages = host
            .process
            .as_ref()
            .map(super::process::Process::client_messages)
            .unwrap_or_default();
        for message in client_messages {
            handle_client_message(&mut host, message);
        }
        if let Some(status) = host.presenter_status.clone() {
            use super::presenter::{PresenterCallback, PresenterCommand};
            let frame = host
                .pending_frame
                .take()
                .unwrap_or(super::windows::WindowsFrame::Events(Vec::new()));
            painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                response.rect,
                PresenterCallback {
                    command: PresenterCommand::Present(frame),
                    status,
                },
            ));
        }
    });
}

pub(crate) fn close(ctx: &egui::Context) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if let Some(mut process) = host.process.take() {
            process.shutdown();
        }
        host.opened = false;
        host.client = None;
        host.block = None;
        if let Some(status) = host.presenter_status.clone() {
            use super::presenter::{PresenterCallback, PresenterCommand};
            ctx.debug_painter()
                .add(eframe::egui_wgpu::Callback::new_paint_callback(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                    PresenterCallback::<super::windows::WindowsFrame> {
                        command: PresenterCommand::Release,
                        status,
                    },
                ));
        }
    });
}

#[derive(Default)]
struct Host {
    process: Option<super::process::Process>,
    input: super::input::InputAdapter,
    presenter_status: Option<super::presenter::PresenterStatus>,
    pending_frame: Option<super::windows::WindowsFrame>,
    client: Option<Arc<block_client::BlockClient>>,
    block: Option<block_client::BlockHandle<Counter>>,
    opened: bool,
}

fn handle_client_message(host: &mut Host, message: DelegatedClientMessage) {
    let Some(client) = &host.client else {
        return;
    };
    let Some(block) = &host.block else {
        return;
    };
    let response = match message {
        DelegatedClientMessage::Watch { request_id, .. } => {
            let Some(data) = client.block_state_bytes(block.id()) else {
                return;
            };
            Message::Client(DelegatedClientMessage::Snapshot {
                instance: EditorInstanceId(1),
                request_id,
                block_id: block.id().into_bytes(),
                author: block.author().unwrap_or(client.account_id()).into_bytes(),
                sequence: block.revision(),
                access: access_byte(client.block_access(block.id())),
                data,
            })
        }
        DelegatedClientMessage::Operate {
            request_id,
            operation_id,
            sequence,
            operation,
            ..
        } => {
            eprintln!(
                "plugin input host received operation request={request_id} sequence={sequence} bytes={}",
                operation.len()
            );
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&operation) else {
                return;
            };
            let Some(value) = value.get("Operate") else {
                return;
            };
            let Ok(operation) = serde_json::from_value::<CounterOperation>(value.clone()) else {
                return;
            };
            block.operate(operation);
            Message::Client(DelegatedClientMessage::Acknowledge {
                instance: EditorInstanceId(1),
                request_id,
                block_id: block.id().into_bytes(),
                operation_id,
                sequence,
            })
        }
        _ => return,
    };
    if let Some(process) = &host.process {
        process.send(vec![response]);
    }
}

fn access_byte(access: block::BlockAccess) -> u8 {
    match access {
        block::BlockAccess::None => 0,
        block::BlockAccess::KnowExists => 1,
        block::BlockAccess::View => 2,
        block::BlockAccess::Edit => 3,
    }
}

fn plugin_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap_or_default();
    path.set_file_name("counter-host.exe");
    path
}
