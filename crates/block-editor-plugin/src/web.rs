use block_plugin_api::{decode_frame, encode_frame, Message};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};

use crate::screens::Screens;

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    static SESSION: RefCell<crate::native::ClientSession> = RefCell::default();
    static SCREENS: RefCell<Option<Rc<RefCell<Screens>>>> = const { RefCell::new(None) };
    static EGUI_CONTEXT: RefCell<Option<eframe::egui::Context>> = const { RefCell::new(None) };
    static CANVAS_ID: RefCell<String> = const { RefCell::new(String::new()) };
    static LAYOUT: RefCell<block_plugin_api::ScreenLayout> = RefCell::default();
}

struct WebApp {
    screens: Rc<RefCell<Screens>>,
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let mut screens = self.screens.borrow_mut();
        for placement in screens.placements() {
            if let Some(session) = screens.session(placement.instance) {
                session.show(ui, placement.region);
            }
        }
    }

    fn raw_input_hook(
        &mut self,
        _context: &eframe::egui::Context,
        input: &mut eframe::egui::RawInput,
    ) {
        let mut screens = self.screens.borrow_mut();
        for placement in screens.placements() {
            if let Some(session) = screens.session(placement.instance) {
                session.append_input(placement.region, input);
            }
        }
    }
}

#[wasm_bindgen(inline_js = "
export function plugin_resize(canvasId, width, height) {
    const canvas = document.getElementById(canvasId);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
}
")]
extern "C" {
    fn plugin_resize(canvas_id: &str, width: f32, height: f32);
}

pub(crate) async fn start<A: crate::App>(canvas_id: String) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let screens = Rc::new(RefCell::new(Screens::new::<A>()));
    let app_screens = Rc::clone(&screens);
    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |creation_context| {
                egui_material_icons::initialize(&creation_context.egui_ctx);
                EGUI_CONTEXT.with(|current| {
                    *current.borrow_mut() = Some(creation_context.egui_ctx.clone());
                });
                Ok(Box::new(WebApp {
                    screens: app_screens,
                }))
            }),
        )
        .await?;
    SESSION.with(|current| *current.borrow_mut() = crate::native::ClientSession::default());
    SCREENS.with(|current| *current.borrow_mut() = Some(screens));
    CANVAS_ID.with(|current| *current.borrow_mut() = canvas_id);
    LAYOUT.with(|current| *current.borrow_mut() = Default::default());
    RUNNER.with(|current| current.replace(Some(runner)));
    Ok(())
}

pub(crate) fn hello(id: &str, name: &str, version: &str) -> Result<Vec<u8>, JsValue> {
    SESSION.with(|session| {
        let client = crate::native::ClientSession::new(id, name, version);
        let frame = encode_frame(&client.hello()).map_err(protocol_error)?;
        *session.borrow_mut() = client;
        Ok(frame)
    })
}

pub(crate) fn receive(frame: Vec<u8>) -> Result<js_sys::Array, JsValue> {
    let message = decode_frame(&frame).map_err(protocol_error)?;
    let mut responses = dispatch(&message);
    responses.extend(SESSION.with(|session| session.borrow_mut().receive(message)));
    responses
        .into_iter()
        .map(|message| {
            encode_frame(&message)
                .map(|frame| JsValue::from(js_sys::Uint8Array::from(frame.as_slice())))
                .map_err(protocol_error)
        })
        .collect()
}

pub(crate) fn poll() -> Result<js_sys::Array, JsValue> {
    let mut messages = Vec::new();
    SCREENS.with(|screens| {
        let Some(screens) = screens.borrow().clone() else {
            return;
        };
        messages.extend(screens.borrow_mut().outbound());
    });
    messages
        .into_iter()
        .map(|message| {
            encode_frame(&message)
                .map(|frame| JsValue::from(js_sys::Uint8Array::from(frame.as_slice())))
                .map_err(protocol_error)
        })
        .collect()
}

pub(crate) fn shutdown() {
    RUNNER.with(|current| {
        if let Some(runner) = current.borrow_mut().take() {
            runner.destroy();
        }
    });
    SCREENS.with(|current| current.borrow_mut().take());
    EGUI_CONTEXT.with(|current| current.borrow_mut().take());
}

fn dispatch(message: &Message) -> Vec<Message> {
    let mut responses = Vec::new();
    SCREENS.with(|screens| {
        let Some(screens) = screens.borrow().clone() else {
            return;
        };
        let mut screens = screens.borrow_mut();
        screens.receive(message);
        let layout = screens.layout().clone();
        if !LAYOUT.with(|current| current.borrow().same_placements(&layout)) {
            let scale = layout
                .screens
                .first()
                .map_or(1.0, block_plugin_api::ScreenPlacement::scale_factor);
            CANVAS_ID.with(|canvas_id| {
                plugin_resize(
                    &canvas_id.borrow(),
                    layout.width as f32 / scale,
                    layout.height as f32 / scale,
                )
            });
            LAYOUT.with(|current| *current.borrow_mut() = layout.clone());
            responses.push(Message::Layout(layout));
        }
        responses.extend(screens.outbound());
    });
    EGUI_CONTEXT.with(|context| {
        if let Some(context) = context.borrow().as_ref() {
            context.request_repaint();
        }
    });
    responses
}

fn protocol_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
