use std::sync::mpsc::{self, Receiver};
use std::thread;

use block_editor_plugin::Waker;

use super::Painted;

pub(super) fn start(data: Vec<u8>, waker: Waker) -> Receiver<Result<Painted, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("paint-review-raster".into())
        .spawn(move || {
            let _ = sender.send(super::paint(&data));
            waker.wake();
        })
        .expect("failed to start the paint rasteriser");
    receiver
}
