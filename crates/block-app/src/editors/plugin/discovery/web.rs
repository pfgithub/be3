use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::Plugins;

const INDEX: &str = "plugins.json";

pub(crate) async fn load() {
    let mut plugins = Plugins::default();
    match get(INDEX).await {
        Ok(document) => match serde_json::from_str::<Vec<String>>(&document) {
            Ok(manifests) => {
                for manifest in manifests {
                    read(&mut plugins, &manifest).await;
                }
            }
            Err(error) => plugins.error(INDEX, error),
        },
        Err(error) => plugins.error(INDEX, error),
    }
    super::install(plugins);
}

async fn read(plugins: &mut Plugins, manifest: &str) {
    match get(manifest).await {
        Ok(document) => plugins.add(manifest, &document),
        Err(error) => plugins.error(manifest, error),
    }
}

async fn get(url: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| describe(&error))?;
    let response: web_sys::Response = response.dyn_into().map_err(|error| describe(&error))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let text = response.text().map_err(|error| describe(&error))?;
    JsFuture::from(text)
        .await
        .map_err(|error| describe(&error))?
        .as_string()
        .ok_or_else(|| "the response was not text".to_owned())
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
