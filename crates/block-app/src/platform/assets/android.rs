use std::{cell::RefCell, ffi::CString, io::Read};

use winit::platform::android::activity::AndroidApp;

thread_local! {
    static APP: RefCell<Option<AndroidApp>> = const { RefCell::new(None) };
}

pub(crate) fn install(app: &AndroidApp) {
    APP.with(|cell| *cell.borrow_mut() = Some(app.clone()));
}

pub(super) fn read(name: &str) -> Result<Vec<u8>, String> {
    APP.with(|cell| {
        let app = cell.borrow();
        let app = app.as_ref().ok_or("the app has no assets")?;
        let path = CString::new(name).map_err(|_| "the asset name is not a C string".to_owned())?;
        let mut asset = app.asset_manager().open(&path).ok_or("no such asset")?;
        let mut bytes = Vec::new();
        asset
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    })
}
