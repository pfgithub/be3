use std::sync::mpsc::{self, Receiver};

use eframe::egui;

use super::{RenderJobResult, RenderTarget};

pub(super) fn spawn_render_job(
    _context: &egui::Context,
    _data: Vec<u8>,
    _page: usize,
    _target: RenderTarget,
) -> Receiver<RenderJobResult> {
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(Err(
        "PDF rendering is not supported on this platform.".to_owned()
    ));
    receiver
}
