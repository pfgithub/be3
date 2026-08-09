use std::sync::Mutex;

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

pub(crate) fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::error_1(&format!("Block panicked: {info}").into());
        *LAST_PANIC
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(info.to_string());
        previous(info);
    }));
}

pub(crate) fn take() -> Option<String> {
    LAST_PANIC
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}
