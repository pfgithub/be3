#[derive(Default)]
pub(super) struct Clipboard {
    #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
            inner: arboard::Clipboard::new().ok(),
        }
    }

    pub(super) fn get(&mut self) -> Option<String> {
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        {
            self.inner.as_mut()?.get_text().ok()
        }
        #[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
        {
            None
        }
    }

    pub(super) fn set(&mut self, text: String) {
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        if let Some(clipboard) = &mut self.inner {
            let _ = clipboard.set_text(text);
        }
        #[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
        let _ = text;
    }
}
