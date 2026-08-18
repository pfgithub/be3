use std::cell::RefCell;

use block::Block;
use block_client::{
    blocks::counter::{Counter, CounterOperation},
    BlockClient, BlockHandle,
};
use block_plugin_api::{
    DelegatedClientMessage, EditorInstanceId, EditorMessage, Message, ViewportMetrics,
};
use eframe::egui;
use std::sync::Arc;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::input::InputAdapter;
use super::presenter::{
    PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, WebFrame,
};
use adapter::WebProtocolAdapter;
const CANVAS_ID: &str = "plugin-demo-canvas";

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    open: bool,
    starting: bool,
    adapter: Option<WebProtocolAdapter>,
    render_available: bool,
    error: Option<String>,
    canvas_size: [u32; 2],
    input: InputAdapter,
    presenter_status: Option<PresenterStatus>,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<Counter>>,
    opened: bool,
    sequence: u64,
    last_count: i64,
    pending_watch: Option<u64>,
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let presenter_status = renderer::install(creation_context);
    let render_available = presenter_status.is_some();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.render_available = render_available;
        state.presenter_status = presenter_status;
        if !render_available {
            state.error = Some("wgpu is not available in this build.".to_owned());
        }
    });
}

pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if state.starting || state.adapter.is_some() || !state.render_available {
            return;
        }
        state.starting = true;
        create_canvas();
        wasm_bindgen_futures::spawn_local(async {
            let result = WebProtocolAdapter::start(CANVAS_ID).await;
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.starting = false;
                match result {
                    Ok(adapter) if state.open => state.adapter = Some(adapter),
                    Ok(mut adapter) => {
                        adapter.shutdown();
                        remove_canvas();
                    }
                    Err(error) => {
                        state.error = Some(error);
                        remove_canvas();
                    }
                }
            });
        });
    });
}

pub(crate) fn editor_ui(ui: &mut egui::Ui, client: Arc<BlockClient>, block: BlockHandle<Counter>) {
    open();
    let ctx = ui.ctx().clone();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.client.get_or_insert(client);
        state.block.get_or_insert(block);
        let presenter_error =
            state
                .presenter_status
                .as_ref()
                .and_then(|status| match status.get() {
                    PresenterState::Unsupported(error) | PresenterState::Failed(error) => {
                        Some(error)
                    }
                    _ => None,
                });
        let error = state.error.clone().or(presenter_error);
        let pixels_per_point = ctx.pixels_per_point();
        if let Some(error) = &error {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Retry").clicked() {
                state.error = None;
                state.open = false;
            }
            return;
        }
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let size = [
            (response.rect.width() * pixels_per_point).round() as u32,
            (response.rect.height() * pixels_per_point).round() as u32,
        ];
        if !state.opened {
            let client = Arc::clone(state.client.as_ref().unwrap());
            let block = state.block.as_ref().unwrap().clone();
            state.sequence = block.revision();
            state.last_count = block.read().map_or(0, |counter| counter.count());
            let message = Message::Editor(EditorMessage::Open {
                instance: EditorInstanceId(1),
                block_id: block.id().into_bytes(),
                block_type: Counter::TYPE_ID.into_bytes(),
                account_id: client.account_id().into_bytes(),
                workspace_id: client.workspace_id().into_bytes(),
                editable: client.block_access(block.id()) == block::BlockAccess::Edit,
                metrics: ViewportMetrics {
                    logical_width: response.rect.width(),
                    logical_height: response.rect.height(),
                    pixel_width: size[0],
                    pixel_height: size[1],
                    scale_factor: pixels_per_point,
                },
            });
            if let Some(adapter) = &mut state.adapter {
                match adapter.send_plugin(message) {
                    Ok(()) => state.opened = true,
                    Err(error) => state.error = Some(error),
                }
            }
        }
        let messages = state
            .adapter
            .is_some()
            .then(|| state.input.update(ui, &response, pixels_per_point));
        if let (Some(adapter), Some(messages)) = (&mut state.adapter, messages) {
            if let Err(error) = adapter.send(messages) {
                state.error = Some(error);
            }
        }
        let client_messages = state
            .adapter
            .as_mut()
            .map(WebProtocolAdapter::take_client_messages)
            .unwrap_or_default();
        for message in client_messages {
            handle_client_message(&mut state, message);
        }
        synchronize_counter(&mut state);
        if let Some(status) = state.presenter_status.clone() {
            painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                response.rect,
                PresenterCallback {
                    command: PresenterCommand::Present(WebFrame {
                        size,
                        canvas_id: CANVAS_ID,
                    }),
                    status,
                },
            ));
        }
        if state.canvas_size != size {
            state.canvas_size = size;
        }
    });
}

pub(crate) fn close(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = false;
        state.opened = false;
        state.client = None;
        state.block = None;
        state.pending_watch = None;
        if let Some(mut adapter) = state.adapter.take() {
            adapter.shutdown();
        }
        state.input = InputAdapter::default();
        state.canvas_size = [0, 0];
        if !state.starting {
            remove_canvas();
        }
        if let Some(status) = state.presenter_status.clone() {
            ctx.debug_painter()
                .add(eframe::egui_wgpu::Callback::new_paint_callback(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                    PresenterCallback::<WebFrame> {
                        command: PresenterCommand::Release,
                        status,
                    },
                ));
        }
    });
}

fn handle_client_message(state: &mut State, message: DelegatedClientMessage) {
    let Some(block) = state.block.clone() else {
        return;
    };
    let response = match message {
        DelegatedClientMessage::Watch { request_id, .. } => {
            let Some(response) = snapshot_message(state, request_id) else {
                state.pending_watch = Some(request_id);
                return;
            };
            response
        }
        DelegatedClientMessage::Operate {
            request_id,
            operation_id,
            sequence,
            operation,
            ..
        } => {
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&operation) else {
                state.error = Some("Counter plugin sent a malformed operation".into());
                return;
            };
            let Some(value) = value.get("Operate") else {
                state.error = Some("Counter plugin sent an unsupported operation".into());
                return;
            };
            let Ok(operation) = serde_json::from_value::<CounterOperation>(value.clone()) else {
                state.error = Some("Counter plugin sent a malformed Counter operation".into());
                return;
            };
            block.operate(operation);
            state.sequence = sequence;
            state.last_count = block
                .read()
                .map_or(state.last_count, |counter| counter.count());
            Message::Client(DelegatedClientMessage::Acknowledge {
                instance: EditorInstanceId(1),
                request_id,
                block_id: block.id().into_bytes(),
                operation_id,
                sequence,
            })
        }
        DelegatedClientMessage::Unwatch { request_id, .. } => {
            Message::Editor(EditorMessage::Acknowledged {
                instance: EditorInstanceId(1),
                request_id,
            })
        }
        _ => return,
    };
    if let Some(adapter) = &mut state.adapter {
        if let Err(error) = adapter.send_plugin(response) {
            state.error = Some(error);
        }
    }
}

fn snapshot_message(state: &State, request_id: u64) -> Option<Message> {
    let client = state.client.as_ref()?;
    let block = state.block.as_ref()?;
    let data = client.block_state_bytes(block.id())?;
    Some(Message::Client(DelegatedClientMessage::Snapshot {
        instance: EditorInstanceId(1),
        request_id,
        block_id: block.id().into_bytes(),
        author: block.author().unwrap_or(client.account_id()).into_bytes(),
        sequence: state.sequence,
        access: access_byte(client.block_access(block.id())),
        data,
    }))
}

fn synchronize_counter(state: &mut State) {
    if let Some(request_id) = state.pending_watch {
        let Some(message) = snapshot_message(state, request_id) else {
            return;
        };
        if send_plugin(state, message) {
            state.pending_watch = None;
        }
        return;
    }
    let Some(block) = state.block.as_ref() else {
        return;
    };
    let Some(count) = block.read().map(|counter| counter.count()) else {
        return;
    };
    if count == state.last_count {
        return;
    }
    let operation = if count > state.last_count {
        CounterOperation::Increment
    } else {
        CounterOperation::Decrement
    };
    state.last_count = state
        .last_count
        .saturating_add(if count > state.last_count { 1 } else { -1 });
    state.sequence = state.sequence.saturating_add(1);
    let operation = serde_json::to_vec(&serde_json::json!({ "Operate": operation })).unwrap();
    let message = Message::Client(DelegatedClientMessage::RemoteOperation {
        instance: EditorInstanceId(1),
        block_id: block.id().into_bytes(),
        operation_id: uuid::Uuid::new_v4().into_bytes(),
        sequence: state.sequence,
        operation,
    });
    send_plugin(state, message);
}

fn send_plugin(state: &mut State, message: Message) -> bool {
    let Some(adapter) = &mut state.adapter else {
        return false;
    };
    match adapter.send_plugin(message) {
        Ok(()) => true,
        Err(error) => {
            state.error = Some(error);
            false
        }
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

fn create_canvas() {
    (|| -> Option<()> {
        let document = web_sys::window()?.document()?;
        if document.get_element_by_id(CANVAS_ID).is_some() {
            return Some(());
        }
        let canvas: web_sys::HtmlCanvasElement =
            document.create_element("canvas").ok()?.dyn_into().ok()?;
        canvas.set_id(CANVAS_ID);
        let _ = canvas.style().set_property("left", "-10000px");
        let _ = canvas.style().set_property("position", "fixed");
        let _ = canvas.style().set_property("top", "0");
        let _ = canvas.style().set_property("visibility", "hidden");
        document.body()?.append_child(&canvas).ok()?;
        Some(())
    })();
}

fn remove_canvas() {
    let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(CANVAS_ID))
    else {
        return;
    };
    canvas.remove();
}
