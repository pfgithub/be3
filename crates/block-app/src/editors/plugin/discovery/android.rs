use std::{ffi::CString, io::Read, sync::Arc};

use winit::platform::android::activity::AndroidApp;

use super::Plugins;

const INDEX: &str = "plugins.json";

pub(crate) fn load(app: &AndroidApp) {
    let assets = app.asset_manager();
    let read = |path: &str| -> Result<Vec<u8>, String> {
        let name = CString::new(path).map_err(|_| "the asset path is not a C string".to_owned())?;
        let mut asset = assets.open(&name).ok_or("no such asset")?;
        let mut bytes = Vec::new();
        asset
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    };
    let mut plugins = Plugins::default();
    match read(INDEX) {
        Ok(document) => match serde_json::from_slice::<Vec<String>>(&document) {
            Ok(manifests) => {
                for manifest in manifests {
                    match read(&manifest).and_then(|bytes| {
                        String::from_utf8(bytes).map_err(|error| error.to_string())
                    }) {
                        Ok(document) => plugins.add(&manifest, &document),
                        Err(error) => plugins.error(&manifest, error),
                    }
                }
            }
            Err(error) => plugins.error(INDEX, error),
        },
        Err(error) => plugins.error(INDEX, error),
    }
    let wanted: Vec<String> = plugins
        .manifests
        .iter()
        .map(|manifest| manifest.entry_point.clone())
        .collect();
    for entry in wanted {
        match read(&compiled(&entry)).or_else(|_| read(&entry)) {
            Ok(bytes) => {
                plugins.modules.insert(entry, Arc::new(bytes));
            }
            Err(error) => plugins.error(&entry, error),
        }
    }
    super::install(plugins);
}

fn compiled(entry: &str) -> String {
    match entry.rsplit_once('.') {
        Some((stem, _)) => format!("{stem}.{}", block_wasm_host::PRECOMPILED_EXTENSION),
        None => entry.to_owned(),
    }
}
