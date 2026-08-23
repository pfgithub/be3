#[cfg(target_os = "android")]
mod android;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

use std::{
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};

use eframe::egui;

#[cfg(target_os = "android")]
use android::open;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use desktop::open;
#[cfg(target_arch = "wasm32")]
use web::open;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Default)]
pub(crate) struct FileFilter {
    #[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
    pub(crate) name: String,
    pub(crate) default_file_name: String,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) extensions: Vec<String>,
    #[cfg_attr(
        all(not(target_os = "android"), not(target_arch = "wasm32")),
        allow(dead_code)
    )]
    pub(crate) mime_types: Vec<String>,
}

impl FileFilter {
    pub(crate) fn new(
        name: &str,
        default_file_name: &str,
        extensions: &[&str],
        mime_types: &[&str],
    ) -> Self {
        Self {
            name: name.to_owned(),
            default_file_name: default_file_name.to_owned(),
            extensions: extensions.iter().map(|it| (*it).to_owned()).collect(),
            mime_types: mime_types.iter().map(|it| (*it).to_owned()).collect(),
        }
    }
}

pub(crate) struct PickedFile {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
}

type PickResult = Result<Option<PickedFile>, String>;

#[derive(Default)]
pub(crate) struct FilePicker {
    pending: Option<Receiver<PickResult>>,
    default_file_name: String,
}

impl FilePicker {
    pub(crate) fn open(&mut self, context: &egui::Context, filter: &FileFilter) {
        self.pending = Some(open(filter));
        self.default_file_name.clone_from(&filter.default_file_name);
        context.request_repaint_after(POLL_INTERVAL);
    }

    pub(crate) fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn poll(&mut self, context: &egui::Context) -> Option<Result<PickedFile, String>> {
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => {
                context.request_repaint_after(POLL_INTERVAL);
                return None;
            }
            Err(TryRecvError::Disconnected) => Ok(None),
        };
        self.pending = None;
        let mut file = result.transpose()?;

        if let Ok(file) = &mut file {
            if file.name.is_empty() {
                file.name.clone_from(&self.default_file_name);
            }
        }
        Some(file)
    }
}
