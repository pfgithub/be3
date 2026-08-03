use std::{
    error::Error,
    future::Future,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use tokio::net::TcpListener;

/// Runs `future` on a scratch runtime on its own thread and delivers its result
/// to the returned channel, so the UI never blocks on the network.
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

/// Starts a block server inside this process and returns the URL it listens on.
pub(crate) fn start_embedded_server(
    data_dir: PathBuf,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let listener = StdTcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    thread::Builder::new()
        .name("block-app-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create embedded block server runtime");
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener)
                    .expect("failed to initialize embedded block server listener");
                if let Err(error) = block_server::serve(listener, data_dir).await {
                    eprintln!("embedded block server stopped: {error}");
                }
            });
        })?;
    Ok(format!("http://{address}"))
}
