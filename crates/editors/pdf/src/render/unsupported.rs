use std::{sync::mpsc, time::Instant};

use block_editor_plugin::{PerformanceReporter, Waker};

use super::{RenderJob, RenderJobMessage, RenderTarget};

pub(crate) fn spawn_render_job(
    _data: Vec<u8>,
    _page: usize,
    _target: RenderTarget,
    _waker: Waker,
    _performance: PerformanceReporter,
) -> RenderJob {
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(RenderJobMessage {
        completed_at: Instant::now(),
        result: Err("PDF rendering is not supported on this platform.".to_owned()),
    });
    RenderJob { receiver }
}
