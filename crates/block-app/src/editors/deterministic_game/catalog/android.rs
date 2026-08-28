use std::{ffi::CString, io::Read};

use winit::platform::android::activity::AndroidApp;

use super::Installed;

const INDEX: &str = "games.json";

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
    match read(INDEX) {
        Ok(document) => match serde_json::from_slice::<Vec<String>>(&document) {
            Ok(modules) => {
                for module in modules {
                    let id = super::identify(&module);
                    match read(&module) {
                        Ok(bytes) => installed.add(&module, &id, &bytes),
                        Err(error) => installed.error(&module, error),
                    }
                }
            }
            Err(error) => installed.error(INDEX, error),
        },
        Err(error) => installed.error(INDEX, error),
    }
    super::install(installed);
}
