use std::sync::mpsc::{self, Receiver};
use std::thread;

use block_editor_plugin::Waker;

use super::Message;

pub(super) fn start(data: Vec<u8>, waker: Waker) -> Receiver<Message> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("paint-review-raster".into())
        .spawn(move || {
            super::paint_all(&data, &mut |message| {
                let _ = sender.send(message);
                waker.wake();
            });
        })
        .expect("failed to start the paint rasteriser");
    receiver
}
