use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::Installed;

const ROOT: &str = "games";

pub(crate) async fn load() {
    let mut installed = Installed::default();
    let index = format!("{ROOT}/index.json");
    match get(&index).await {
        Ok(document) => match serde_json::from_slice::<Vec<String>>(&document) {
            Ok(modules) => {
                for module in modules {
                    read(&mut installed, &module).await;
                }
            }
            Err(error) => installed.error(&index, error),
        },
        Err(error) => installed.error(&index, error),
    }
    super::install(installed);
}

async fn read(installed: &mut Installed, module: &str) {
    let source = format!("{ROOT}/{module}");
    let id = module.strip_suffix(".wasm").unwrap_or(module).to_owned();
    match get(&source).await {
        Ok(bytes) => installed.add(&source, &id, &bytes),
        Err(error) => installed.error(&source, error),
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
