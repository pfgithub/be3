use std::{
    future::Future,
    sync::mpsc::{self, Receiver},
};

/// Runs `future` as a task on the browser's event loop and delivers its result
/// to the returned channel. There is no thread to move it to, so the request
/// makes progress between frames rather than alongside them.
///
/// The future is not required to be `Send`, since it never leaves this thread,
/// but the bound matches the native signature so callers can be shared.
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
