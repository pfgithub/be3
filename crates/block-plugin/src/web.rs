use block_plugin_api::{decode_frame, encode_frame, Message};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    static SESSION: RefCell<crate::native::ClientSession> = RefCell::default();
    static EGUI_SESSION: RefCell<Option<Rc<RefCell<crate::egui_session::EguiSession>>>> = const { RefCell::new(None) };
    static EGUI_CONTEXT: RefCell<Option<eframe::egui::Context>> = const { RefCell::new(None) };
    static CANVAS_ID: RefCell<String> = const { RefCell::new(String::new()) };
}

struct WebApp {
    session: Rc<RefCell<crate::egui_session::EguiSession>>,
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.session.borrow_mut().show(ui);
    }

    fn raw_input_hook(
        &mut self,
        _context: &eframe::egui::Context,
        input: &mut eframe::egui::RawInput,
    ) {
        self.session.borrow_mut().append_input(input);
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
    let session = Rc::new(RefCell::new(crate::egui_session::EguiSession::new::<A>()));
    let app_session = Rc::clone(&session);
    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |creation_context| {
                EGUI_CONTEXT.with(|current| {
                    *current.borrow_mut() = Some(creation_context.egui_ctx.clone());
                });
                Ok(Box::new(WebApp {
                    session: app_session,
                }))
            }),
        )
        .await?;
    SESSION.with(|current| *current.borrow_mut() = crate::native::ClientSession::default());
    EGUI_SESSION.with(|current| *current.borrow_mut() = Some(session));
    CANVAS_ID.with(|current| *current.borrow_mut() = canvas_id);
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
    dispatch(&message);
    SESSION.with(|session| {
        session
            .borrow_mut()
            .receive(message)
            .into_iter()
            .map(|message| {
                encode_frame(&message)
                    .map(|frame| JsValue::from(js_sys::Uint8Array::from(frame.as_slice())))
                    .map_err(protocol_error)
            })
            .collect()
    })
}

pub(crate) fn shutdown() {
    RUNNER.with(|current| {
        if let Some(runner) = current.borrow_mut().take() {
            runner.destroy();
        }
    });
    EGUI_SESSION.with(|current| current.borrow_mut().take());
    EGUI_CONTEXT.with(|current| current.borrow_mut().take());
}

fn dispatch(message: &Message) {
    if let Some(metrics) = match message {
        Message::CreateViewport(viewport) => Some(&viewport.metrics),
        Message::ResizeViewport(metrics) => Some(metrics),
        _ => None,
    } {
        CANVAS_ID.with(|canvas_id| {
            plugin_resize(
                &canvas_id.borrow(),
                metrics.logical_width,
                metrics.logical_height,
            );
        });
    }
    EGUI_SESSION.with(|session| {
        if let Some(session) = session.borrow_mut().as_mut() {
            session.borrow_mut().receive(message);
        }
    });
    EGUI_CONTEXT.with(|context| {
        if let Some(context) = context.borrow().as_ref() {
            context.request_repaint();
        }
    });
}

fn protocol_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
