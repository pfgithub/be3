use std::sync::mpsc;

use super::{RenderJob, RenderTarget};

pub(crate) fn spawn_render_job(_data: Vec<u8>, _page: usize, _target: RenderTarget) -> RenderJob {
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(Err(
        "PDF rendering is not supported on this platform.".to_owned()
    ));
    receiver
}
