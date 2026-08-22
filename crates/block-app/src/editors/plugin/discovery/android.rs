use std::{ffi::CString, io::Read, path::PathBuf};

use winit::platform::android::activity::AndroidApp;

use super::Plugins;

const ROOT: &str = "plugins";

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
    let index = format!("{ROOT}/index.json");
    match read(&index) {
        Ok(document) => match serde_json::from_str::<Vec<String>>(&document) {
            Ok(directories) => {
                for directory in directories {
                    let source = format!("{ROOT}/{directory}/manifest.json");
                    match read(&source) {
                        Ok(document) => plugins.add(
                            &source,
                            PathBuf::from(format!("{ROOT}/{directory}")),
                            &document,
                        ),
                        Err(error) => plugins.error(&source, error),
                    }
                }
            }
            Err(error) => plugins.error(&index, error),
        },
        Err(error) => plugins.error(&index, error),
    }
    super::install(plugins);
}
