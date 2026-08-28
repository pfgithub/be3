use std::sync::mpsc::{self, Receiver};

use block_editor_plugin::Waker;

use super::Message;

pub(super) fn start(data: Vec<u8>, waker: Waker) -> Receiver<Message> {
    let (sender, receiver) = mpsc::channel();
    super::paint_all(&data, &mut |message| {
        let _ = sender.send(message);
    });
    waker.wake();
    receiver
}
