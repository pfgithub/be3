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

pub(crate) enum SocketMessage {
    Text(String),

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Ping(usize),
    Close,
}
