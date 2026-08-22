use std::{ffi::CString, io::Read};

use winit::platform::android::activity::AndroidApp;

use super::Installed;

const ROOT: &str = "games";

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
    let mut installed = Installed::default();
    let index = format!("{ROOT}/index.json");
    match read(&index) {
        Ok(document) => match serde_json::from_slice::<Vec<String>>(&document) {
            Ok(modules) => {
                for module in modules {
                    let source = format!("{ROOT}/{module}");
                    let id = module.strip_suffix(".wasm").unwrap_or(&module).to_owned();
                    match read(&source) {
                        Ok(bytes) => installed.add(&source, &id, &bytes),
                        Err(error) => installed.error(&source, error),
                    }
                }
            }
            Err(error) => installed.error(&index, error),
        },
        Err(error) => installed.error(&index, error),
    }
    super::install(installed);
}
