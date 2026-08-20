//! Platform transports for the block client.
//!
//! The worker's protocol logic is identical everywhere; only the parts that
//! reach the network differ. Native builds use a dedicated thread running a
//! Tokio runtime with `tokio-tungstenite` and `ureq`; the web build runs the
//! same worker as a task on the browser's event loop and reaches the network
//! with the `WebSocket` and `fetch` APIs.

#[cfg(not(target_arch = "wasm32"))]
mod native;
mod tunnel;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{post_json, spawn_worker, Socket};
pub(crate) use tunnel::TunnelSocket;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{post_json, spawn_worker, Socket};

/// Whatever a worker talks to the server through: its own websocket, or
/// another client's connection.
pub(crate) enum Link {
    Socket(Socket),
    Tunnel(TunnelSocket),
}

impl Link {
    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), String> {
        match self {
            Self::Socket(socket) => socket.send_text(text).await,
            Self::Tunnel(tunnel) => tunnel.send_text(text).await,
        }
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        match self {
            Self::Socket(socket) => socket.next().await,
            Self::Tunnel(tunnel) => tunnel.next().await,
        }
    }
}

/// A frame read from the block websocket.
pub(crate) enum SocketMessage {
    Text(String),
    /// A ping the transport has already answered, carrying the payload size so
    /// the traffic log can still show it. Browsers answer pings inside the
    /// websocket implementation and never report them, so this is native-only.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Ping(usize),
    Close,
}
