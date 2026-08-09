use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// A GET request running on the browser's event loop, polled from the UI
/// thread each frame since egui does not drive futures itself.
pub(in crate::debug::version) struct Fetch {
    state: Rc<RefCell<Option<Result<Vec<u8>, String>>>>,
}

impl Fetch {
    pub(in crate::debug::version) fn get(
        url: String,
        headers: Vec<(&'static str, String)>,
    ) -> Self {
        let state = Rc::new(RefCell::new(None));
        let state_for_task = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = run(url, headers).await;
            *state_for_task.borrow_mut() = Some(result);
        });
        Self { state }
    }

    pub(in crate::debug::version) fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        self.state.borrow_mut().take()
    }
}

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

async fn run(url: String, headers: Vec<(&'static str, String)>) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no browser window is available")?;

    let js_headers = web_sys::Headers::new()
        .map_err(|error| format!("failed to build request headers: {}", describe(&error)))?;
    for (key, value) in &headers {
        js_headers.append(key, value).map_err(|error| {
            format!(
                "failed to set the {key} request header: {}",
                describe(&error)
            )
        })?;
    }

    let options = web_sys::RequestInit::new();
    options.set_method("GET");
    options.set_headers(&js_headers);

    let request = web_sys::Request::new_with_str_and_init(&url, &options)
        .map_err(|error| format!("invalid URL {url}: {}", describe(&error)))?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|error| format!("request failed: {}", describe(&error)))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|error| format!("unexpected fetch result: {}", describe(&error)))?;

    let status = response.status();
    let buffer = response
        .array_buffer()
        .map_err(|error| format!("unreadable response: {}", describe(&error)))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|error| format!("unreadable response: {}", describe(&error)))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();

    if !(200..300).contains(&status) {
        return Err(format!(
            "HTTP {status}: {}",
            String::from_utf8_lossy(&bytes)
        ));
    }
    Ok(bytes)
}
