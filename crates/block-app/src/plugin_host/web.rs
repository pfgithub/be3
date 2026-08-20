use std::cell::RefCell;

use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::{EditorInstanceId, EditorMessage, Message, ScreenLayout};
use eframe::egui;
use std::sync::Arc;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::{
    instances::{Instances, NextScreens},
    presenter::{PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, Region},
};
use adapter::WebProtocolAdapter;
const CANVAS_ID: &str = "counter-canvas";

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
    instances: Instances,
    layout: ScreenLayout,
    sent: Vec<block_plugin_api::ScreenRequest>,
    pass: u64,
    presenter_status: Option<PresenterStatus>,
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

pub(crate) fn editor_ui(
    ui: &mut egui::Ui,
    client: Arc<BlockClient>,
    block: BlockHandle<Counter>,
    instance: EditorInstanceId,
) {
    open();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        begin_pass(&mut state, ui.ctx());
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
        let pass = state.pass;
        let screen = state.instances.report(
            instance,
            client,
            block,
            response.rect.size(),
            ui.ctx().pixels_per_point(),
            pass,
        );
        let messages = state
            .instances
            .input(instance, |input| input.update(ui, &response, screen));
        send(&mut state, messages);
        let Some(status) = state.presenter_status.clone() else {
            return;
        };
        let Some(region) = Region::of(&state.layout, screen) else {
            return;
        };
        let size = [state.layout.width, state.layout.height];
        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            PresenterCallback {
                command: PresenterCommand::Present(renderer::WebFrame {
                    size,
                    canvas_id: CANVAS_ID,
                }),
                status,
                region,
            },
        ));
    });
}

pub(crate) fn close(ctx: &egui::Context, instance: EditorInstanceId) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.instances.remove(instance) {
            return;
        }
        send(
            &mut state,
            vec![Message::Editor(EditorMessage::Close { instance })],
        );
        if !state.instances.is_empty() {
            return;
        }
        state.open = false;
        state.layout = ScreenLayout::default();
        state.sent.clear();
        if let Some(mut adapter) = state.adapter.take() {
            adapter.shutdown();
        }
        if !state.starting {
            remove_canvas();
        }
        if let Some(status) = state.presenter_status.clone() {
            ctx.debug_painter()
                .add(eframe::egui_wgpu::Callback::new_paint_callback(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                    PresenterCallback::<renderer::WebFrame> {
                        command: PresenterCommand::Release,
                        status,
                        region: Region::default(),
                    },
                ));
        }
    });
}

fn begin_pass(state: &mut State, ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    if state.pass == pass {
        return;
    }
    let previous = state.pass;
    state.pass = pass;
    if state.adapter.is_none() {
        return;
    }
    let NextScreens { opened, screens } = state.instances.next_screens(previous);
    let mut messages = opened;
    if state.sent != screens {
        state.sent.clone_from(&screens);
        messages.push(state.instances.screen_set(screens));
    }
    messages.extend(state.instances.pending());
    send(state, messages);
    let client_messages = state
        .adapter
        .as_mut()
        .map(WebProtocolAdapter::take_client_messages)
        .unwrap_or_default();
    if let Some(layout) = state
        .adapter
        .as_mut()
        .and_then(WebProtocolAdapter::take_layout)
    {
        state.layout = layout;
    }
    for message in client_messages {
        state.instances.client_message(message);
    }
}

fn send(state: &mut State, messages: Vec<Message>) {
    if messages.is_empty() {
        return;
    }
    let Some(adapter) = &mut state.adapter else {
        return;
    };
    if let Err(error) = adapter.send(messages) {
        state.error = Some(error);
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
