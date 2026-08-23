use std::{future::Future, thread, time::Duration};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};

use super::SocketMessage;
use crate::fatal;

type Stream = WebSocketStream<MaybeTlsStream<TcpStream>>;

const MANAGEMENT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_WEBSOCKET_FRAME_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn spawn_worker<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    thread::Builder::new()
        .name("block-client".into())
        .spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|error| {
                fatal(format!("failed to create block client runtime: {error}"))
            });
            runtime.block_on(future);
        })
        .unwrap_or_else(|error| fatal(format!("failed to spawn block client worker: {error}")));
}

pub(crate) struct Socket {
    sink: SplitSink<Stream, Message>,
    source: SplitStream<Stream>,
}

impl Socket {
    pub(crate) async fn connect(url: &str) -> Result<Self, String> {
        let request = url
            .into_client_request()
            .map_err(|error| format!("invalid block server URL {url}: {error}"))?;
        let (socket, _) = connect_async_with_config(
            request,
            Some(WebSocketConfig {
                max_frame_size: Some(MAX_WEBSOCKET_FRAME_BYTES),
                ..WebSocketConfig::default()
            }),
            false,
        )
        .await
        .map_err(|error| format!("failed to connect to {url}: {error}"))?;
        let (sink, source) = socket.split();
        Ok(Self { sink, source })
    }

    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), String> {
        self.sink
            .send(Message::Text(text))
            .await
            .map_err(|error| format!("failed to send block message: {error}"))
    }

    pub(crate) async fn next(&mut self) -> Option<Result<SocketMessage, String>> {
        let message = match self.source.next().await? {
            Ok(message) => message,
            Err(error) => return Some(Err(format!("block server connection failed: {error}"))),
        };
        match message {
            Message::Text(text) => Some(Ok(SocketMessage::Text(text))),
            Message::Ping(payload) => {
                let length = payload.len();
                if let Err(error) = self.sink.send(Message::Pong(payload)).await {
                    return Some(Err(format!("failed to send pong: {error}")));
                }
                Some(Ok(SocketMessage::Ping(length)))
            }
            Message::Close(_) => Some(Ok(SocketMessage::Close)),
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                Some(Err("server sent an unsupported websocket message".into()))
            }
        }
    }
}

pub(crate) async fn post_json(url: String, body: Vec<u8>) -> Result<(u16, String), String> {
    tokio::task::spawn_blocking(move || {
        let agent = ureq::AgentBuilder::new()
            .timeout(MANAGEMENT_TIMEOUT)
            .build();
        let response = match agent
            .post(&url)
            .set("content-type", "application/json")
            .send_bytes(&body)
        {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(error) => return Err(error.to_string()),
        };
        let status = response.status();
        response
            .into_string()
            .map(|text| (status, text))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
