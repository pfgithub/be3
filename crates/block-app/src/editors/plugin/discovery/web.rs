use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::Plugins;

const ROOT: &str = "plugins";

pub(crate) async fn load() {
    let mut plugins = Plugins::default();
    let index = format!("{ROOT}/index.json");
    match get(&index).await {
        Ok(document) => match serde_json::from_str::<Vec<String>>(&document) {
            Ok(directories) => {
                for directory in directories {
                    read(&mut plugins, &directory).await;
                }
            }
            Err(error) => plugins.error(&index, error),
        },
        Err(error) => plugins.error(&index, error),
    }
    super::install(plugins);
}

async fn read(plugins: &mut Plugins, directory: &str) {
    let directory = format!("{ROOT}/{directory}");
    let source = format!("{directory}/manifest.json");
    match get(&source).await {
        Ok(document) => plugins.add(&source, directory, &document),
        Err(error) => plugins.error(&source, error),
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
