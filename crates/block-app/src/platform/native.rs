use std::{
    error::Error,
    future::Future,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use tokio::net::TcpListener;

pub(crate) fn spawn_request<T>(future: impl Future<Output = T> + Send + 'static) -> Receiver<T>
where
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("block-app-request".into())
        .spawn(move || {
            let result = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime.block_on(future),
                Err(error) => panic!("failed to start a request runtime: {error}"),
            };
            let _ = sender.send(result);
        })
        .unwrap_or_else(|error| panic!("failed to start request: {error}"));
    receiver
}

pub(crate) struct EmbeddedServer {
    pub(crate) url: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) fn start_embedded_server(
    data_dir: PathBuf,
) -> Result<EmbeddedServer, Box<dyn Error + Send + Sync>> {
    let listener = StdTcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let thread = thread::Builder::new()
        .name("block-app-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create embedded block server runtime");
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener)
                    .expect("failed to initialize embedded block server listener");
                let shutdown = async {
                    let _ = shutdown_receiver.await;
                };
                if let Err(error) = block_server::serve_until_shutdown(
                    listener,
                    data_dir,
                    block_server::ServerConfig::default(),
                    shutdown,
                )
                .await
                {
                    panic!("embedded block server stopped: {error}");
                }
            });
        })?;
    Ok(EmbeddedServer {
        url: format!("http://{address}"),
        shutdown: Some(shutdown_sender),
        thread: Some(thread),
    })
}
