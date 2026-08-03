//! Platform transports for the block client.
//!
//! The worker's protocol logic is identical everywhere; only the parts that
//! reach the network differ. Native builds use a dedicated thread running a
//! Tokio runtime with `tokio-tungstenite` and `ureq`; the web build runs the
//! same worker as a task on the browser's event loop and reaches the network
//! with the `WebSocket` and `fetch` APIs.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{post_json, spawn_worker, Socket};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{post_json, spawn_worker, Socket};

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
