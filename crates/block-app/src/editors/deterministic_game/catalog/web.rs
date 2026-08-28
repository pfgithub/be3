use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::Installed;

const INDEX: &str = "games.json";

pub(crate) async fn load() {
    let mut installed = Installed::default();
    match get(INDEX).await {
        Ok(document) => match serde_json::from_slice::<Vec<String>>(&document) {
            Ok(modules) => {
                for module in modules {
                    read(&mut installed, &module).await;
                }
            }
            Err(error) => installed.error(INDEX, error),
        },
        Err(error) => installed.error(INDEX, error),
    }
    super::install(installed);
}

async fn read(installed: &mut Installed, module: &str) {
    let id = super::identify(module);
    match get(module).await {
        Ok(bytes) => installed.add(module, &id, &bytes),
        Err(error) => installed.error(module, error),
    }
}

async fn get(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| describe(&error))?;
    let response: web_sys::Response = response.dyn_into().map_err(|error| describe(&error))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = response.array_buffer().map_err(|error| describe(&error))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|error| describe(&error))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
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
