use std::sync::mpsc;

use block_editor_plugin::Waker;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::{Download, Painting};

pub(super) fn start(waker: Waker) -> Download {
    let (sender, receiver) = mpsc::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = sender.send(download().await);
        waker.wake();
    });
    Download { receiver }
}

async fn download() -> Result<Vec<Painting>, String> {
    let tree = get(&super::tree_url()).await?;
    let mut paintings = Vec::new();
    for path in super::paths_in(&tree)? {
        let data = get(&super::file_url(&path)).await?;
        paintings.push(super::painting(path, data));
    }
    Ok(paintings)
}

async fn get(url: &str) -> Result<Vec<u8>, String> {
    let response = JsFuture::from(fetch(url)?)
        .await
        .map_err(|error| format!("could not download {url}: {}", describe(&error)))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|error| format!("unexpected answer for {url}: {}", describe(&error)))?;
    if !response.ok() {
        return Err(format!("{url} answered {}", response.status()));
    }
    let buffer = response
        .array_buffer()
        .map_err(|error| format!("could not read {url}: {}", describe(&error)))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|error| format!("could not read {url}: {}", describe(&error)))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn fetch(url: &str) -> Result<js_sys::Promise, String> {
    let global = js_sys::global();
    if let Some(scope) = global.dyn_ref::<web_sys::WorkerGlobalScope>() {
        return Ok(scope.fetch_with_str(url));
    }
    if let Some(window) = global.dyn_ref::<web_sys::Window>() {
        return Ok(window.fetch_with_str(url));
    }
    Err("nothing here can reach the network".to_owned())
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
