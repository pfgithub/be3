use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Waker},
};

use super::SocketMessage;

const NO_NETWORK: &str = "a hosted block client reaches the server through its host, not directly";

type Task = Pin<Box<dyn Future<Output = ()>>>;

thread_local! {
    static TASKS: RefCell<Vec<Task>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn spawn_worker<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    TASKS.with(|tasks| tasks.borrow_mut().push(Box::pin(future)));
}

pub fn pump() {
    let mut context = Context::from_waker(Waker::noop());
    let mut running = TASKS.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));
    running.retain_mut(|task| task.as_mut().poll(&mut context).is_pending());
    TASKS.with(|tasks| {
        let mut tasks = tasks.borrow_mut();
        running.append(&mut tasks);
        *tasks = running;
    });
}

pub(crate) struct Socket;

impl Socket {
    pub(crate) async fn connect(_url: &str) -> Result<Self, String> {
        Err(NO_NETWORK.to_owned())
    }

    pub(crate) async fn send_text(&mut self, _text: String) -> Result<(), String> {
        Err(NO_NETWORK.to_owned())
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        Some(Err(NO_NETWORK.to_owned()))
    }
}

pub(crate) async fn post_json(_url: String, _body: Vec<u8>) -> Result<(u16, String), String> {
    Err(NO_NETWORK.to_owned())
}
