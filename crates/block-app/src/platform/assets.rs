#[cfg(target_os = "android")]
mod android;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod native;

#[cfg(target_os = "android")]
pub(crate) use android::install;

const REFUSED: &str = "an asset is a relative path inside the app's own directory:";

pub(crate) struct Asset {
    #[cfg(not(target_arch = "wasm32"))]
    result: Option<Result<Vec<u8>, String>>,
    #[cfg(target_arch = "wasm32")]
    fetch: super::http::Fetch,
}

impl Asset {
    pub(crate) fn read(name: &str) -> Self {
        match inside_the_app(name) {
            true => Self::open(name),
            false => Self::refused(format!("{REFUSED} {name}")),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn refused(reason: String) -> Self {
        Self {
            result: Some(Err(reason)),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn refused(reason: String) -> Self {
        Self {
            fetch: super::http::Fetch::refused(reason),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open(name: &str) -> Self {
        Self {
            result: Some(read(name)),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn open(name: &str) -> Self {
        Self {
            fetch: super::http::Fetch::get(name.to_owned(), Vec::new()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn poll(&mut self) -> Option<Result<Vec<u8>, String>> {
        self.result.take()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn poll(&mut self) -> Option<Result<Vec<u8>, String>> {
        self.fetch.poll()
    }
}

#[cfg(target_os = "android")]
fn read(name: &str) -> Result<Vec<u8>, String> {
    android::read(name)
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn read(name: &str) -> Result<Vec<u8>, String> {
    native::read(name)
}

fn inside_the_app(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':'])
        && name
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests;
