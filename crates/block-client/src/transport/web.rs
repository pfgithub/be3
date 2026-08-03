use std::{cell::RefCell, future::Future, rc::Rc};

use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::SocketMessage;

/// Runs the block client worker as a task on the browser's event loop. There is
/// no thread to move it to, so the worker shares the main thread with the UI
/// and only makes progress while the UI is between frames.
pub(crate) fn spawn_worker<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

/// Describes a `JsValue` error for a message, which is otherwise opaque.
fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{error:?}"))
}

pub(crate) struct Socket {
    socket: web_sys::WebSocket,
    events: mpsc::UnboundedReceiver<Result<SocketMessage, String>>,
    /// The event handlers are owned here: dropping a `Closure` invalidates the
    /// function the browser holds, so they must outlive the socket.
    _handlers: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl Socket {
    pub(crate) async fn connect(url: &str) -> Result<Self, String> {
        let socket = web_sys::WebSocket::new(url)
            .map_err(|error| format!("failed to connect to {url}: {}", describe(&error)))?;
        let (events, events_rx) = mpsc::unbounded();
        let (opened, opened_rx) = oneshot::channel();
        // The handshake finishes with whichever of `open`, `error`, or `close`
        // fires first, and each may only report once.
        let opened = Rc::new(RefCell::new(Some(opened)));

        let on_message = {
            let events = events.clone();
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let message = match event.data().as_string() {
                    Some(text) => Ok(SocketMessage::Text(text)),
                    None => Err("server sent an unsupported websocket message".to_owned()),
                };
                let _ = events.unbounded_send(message);
            })
        };
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let on_open = {
            let opened = Rc::clone(&opened);
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                if let Some(opened) = opened.borrow_mut().take() {
                    let _ = opened.send(Ok(()));
                }
            })
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let on_error = {
            let events = events.clone();
            let opened = Rc::clone(&opened);
            let url = url.to_owned();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                // The browser does not report why a websocket failed, to keep
                // cross-origin failures from leaking information to the page.
                let message = format!("block server connection to {url} failed");
                match opened.borrow_mut().take() {
                    Some(opened) => {
                        let _ = opened.send(Err(message));
                    }
                    None => {
                        let _ = events.unbounded_send(Err(message));
                    }
                }
            })
        };
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        let on_close = {
            let events = events.clone();
            let opened = Rc::clone(&opened);
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
                match opened.borrow_mut().take() {
                    Some(opened) => {
                        let _ = opened.send(Err("block server closed the connection".to_owned()));
                    }
                    None => {
                        let _ = events.unbounded_send(Ok(SocketMessage::Close));
                    }
                }
            })
        };
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        opened_rx
            .await
            .map_err(|_| "websocket handshake was abandoned".to_owned())??;

        Ok(Self {
            socket,
            events: events_rx,
            _handlers: vec![on_open, on_error, on_close],
            _on_message: on_message,
        })
    }

    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), String> {
        self.socket
            .send_with_str(&text)
            .map_err(|error| format!("failed to send block message: {}", describe(&error)))
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        self.events.next().await
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        // Detach the handlers before their closures are dropped, so a late
        // event cannot reach freed functions.
        self.socket.set_onmessage(None);
        self.socket.set_onopen(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

/// POSTs a JSON body and returns the response status and body.
///
/// `fetch` has no timeout of its own, so management requests wait as long as
/// the browser keeps the request alive rather than the native client's
/// `MANAGEMENT_TIMEOUT`.
pub(crate) async fn post_json(url: String, body: Vec<u8>) -> Result<(u16, String), String> {
    let window = web_sys::window().ok_or("no browser window is available")?;

    let headers = web_sys::Headers::new()
        .map_err(|error| format!("failed to build request headers: {}", describe(&error)))?;
    headers
        .append("content-type", "application/json")
        .map_err(|error| {
            format!(
                "failed to set the request content type: {}",
                describe(&error)
            )
        })?;

    let options = web_sys::RequestInit::new();
    options.set_method("POST");
    options.set_headers(&headers);
    options.set_body(&js_sys::Uint8Array::from(body.as_slice()));

    let request = web_sys::Request::new_with_str_and_init(&url, &options)
        .map_err(|error| format!("invalid management URL {url}: {}", describe(&error)))?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|error| format!("management request failed: {}", describe(&error)))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|error| format!("unexpected fetch result: {}", describe(&error)))?;

    let status = response.status();
    let text = response
        .text()
        .map_err(|error| format!("unreadable management response: {}", describe(&error)))?;
    let text = JsFuture::from(text)
        .await
        .map_err(|error| format!("unreadable management response: {}", describe(&error)))?;
    Ok((status, text.as_string().unwrap_or_default()))
}
