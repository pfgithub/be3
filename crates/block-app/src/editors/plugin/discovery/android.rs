use std::{ffi::CString, io::Read};

use winit::platform::android::activity::AndroidApp;

use super::Plugins;

const INDEX: &str = "plugins.json";

pub(crate) fn load(app: &AndroidApp) {
    let assets = app.asset_manager();
    let read = |path: &str| -> Result<String, String> {
        let name = CString::new(path).map_err(|_| "the asset path is not a C string".to_owned())?;
        let mut asset = assets.open(&name).ok_or("no such asset")?;
        let mut document = String::new();
        asset
            .read_to_string(&mut document)
            .map_err(|error| error.to_string())?;
        Ok(document)
    };
    let mut plugins = Plugins::default();
    match read(INDEX) {
        Ok(document) => match serde_json::from_str::<Vec<String>>(&document) {
            Ok(manifests) => {
                for manifest in manifests {
                    match read(&manifest) {
                        Ok(document) => plugins.add(&manifest, &document),
                        Err(error) => plugins.error(&manifest, error),
                    }
                }
            }
            Err(error) => plugins.error(INDEX, error),
        },
        Err(error) => plugins.error(INDEX, error),
    }
    super::install(plugins);
}
