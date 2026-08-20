//! Choosing a file to import, on every platform the app runs on.
//!
//! Desktop opens a modal dialog through `rfd` and has an answer by the time it
//! returns. Android and the browser cannot work that way: Android hands the
//! request to the system document picker and hears back through `MainActivity`
//! once the user returns to the app, and the browser drives a hidden
//! `<input type="file">` whose events arrive on later ticks of the event loop.
//! So a picker is opened once and then polled every frame until it answers.

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

/// How often a pending picker asks for another frame. Nothing here is driven by
/// egui, so without this the answer would sit unread until the next input.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// What a picker offers to choose from. Each platform reads the part of this it
/// can use: a desktop dialog filters by extension, Android matches MIME types,
/// and a browser accepts both.
pub(crate) struct FileFilter {
    #[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
    pub(crate) name: &'static str,
    pub(crate) default_file_name: &'static str,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) extensions: &'static [&'static str],
    #[cfg_attr(
        all(not(target_os = "android"), not(target_arch = "wasm32")),
        allow(dead_code)
    )]
    pub(crate) mime_types: &'static [&'static str],
}

pub(crate) struct PickedFile {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
}

type PickResult = Result<Option<PickedFile>, String>;

#[derive(Default)]
pub(crate) struct FilePicker {
    pending: Option<Receiver<PickResult>>,
    default_file_name: &'static str,
}

impl FilePicker {
    pub(crate) fn open(&mut self, context: &egui::Context, filter: &'static FileFilter) {
        self.pending = Some(open(filter));
        self.default_file_name = filter.default_file_name;
        context.request_repaint_after(POLL_INTERVAL);
    }

    pub(crate) fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Returns the chosen file once the picker closes, or why it could not be
    /// read. A cancelled picker reports nothing at all.
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
        // A document provider does not have to name what it hands over, so a
        // block always gets something readable to show instead.
        if let Ok(file) = &mut file {
            if file.name.is_empty() {
                file.name = self.default_file_name.to_owned();
            }
        }
        Some(file)
    }
}
