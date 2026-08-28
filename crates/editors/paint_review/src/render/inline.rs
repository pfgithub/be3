use std::sync::mpsc::{self, Receiver};

use block_editor_plugin::Waker;

use super::Painted;

pub(super) fn start(data: Vec<u8>, waker: Waker) -> Receiver<Result<Painted, String>> {
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(super::paint(&data));
    waker.wake();
    receiver
}
