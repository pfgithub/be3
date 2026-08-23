use std::{
    future::Future,
    sync::mpsc::{self, Receiver},
};

pub(crate) fn spawn_request<T>(future: impl Future<Output = T> + 'static) -> Receiver<T>
where
    T: 'static,
{
    let (sender, receiver) = mpsc::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = sender.send(future.await);
    });
    receiver
}
