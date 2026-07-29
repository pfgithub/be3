#![allow(dead_code)]

use std::path::PathBuf;

use block::{ClientMessage, ReferenceDelta, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::{fs, net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestServer {
    pub root: PathBuf,
    pub url: String,
    task: JoinHandle<()>,
}

impl TestServer {
    pub async fn start() -> Self {
        let root = std::env::temp_dir().join(format!("block-dependencies-test-{}", Uuid::new_v4()));
        Self::start_at(root).await
    }

    pub async fn start_at(root: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server_root = root.clone();
        let task = tokio::spawn(async move {
            block_server::serve(listener, server_root).await.unwrap();
        });
        Self { root, url, task }
    }

    pub async fn connect(&self) -> Socket {
        connect_async(&self.url).await.unwrap().0
    }

    pub async fn stop(self) -> PathBuf {
        self.task.abort();
        let _ = self.task.await;
        self.root
    }

    pub async fn cleanup(self) {
        let root = self.stop().await;
        fs::remove_dir_all(root).await.unwrap();
    }
}

pub async fn request(socket: &mut Socket, message: ClientMessage) -> ServerMessage {
    socket
        .send(Message::Text(serde_json::to_string(&message).unwrap()))
        .await
        .unwrap();
    let message = socket.next().await.unwrap().unwrap();
    serde_json::from_str(&message.into_text().unwrap()).unwrap()
}

pub async fn create(socket: &mut Socket, id: Uuid, references: Vec<Uuid>) -> ServerMessage {
    request(
        socket,
        ClientMessage::CreateBlock {
            request_id: Uuid::new_v4(),
            id,
            block_type: Uuid::new_v4(),
            data: vec![],
            references,
            watch: false,
        },
    )
    .await
}

pub async fn update(
    socket: &mut Socket,
    id: Uuid,
    added: Vec<Uuid>,
    removed: Vec<Uuid>,
) -> ServerMessage {
    request(
        socket,
        ClientMessage::UpdateBlock {
            request_id: Uuid::new_v4(),
            id,
            seq: None,
            operation_id: Uuid::new_v4(),
            operation: vec![],
            references: ReferenceDelta { added, removed },
        },
    )
    .await
}

pub async fn set_parent(socket: &mut Socket, id: Uuid, parent: Option<Uuid>) -> ServerMessage {
    request(
        socket,
        ClientMessage::SetBlockParent {
            request_id: Uuid::new_v4(),
            id,
            parent,
        },
    )
    .await
}

pub async fn read(socket: &mut Socket, id: Uuid) -> ServerMessage {
    request(
        socket,
        ClientMessage::ReadBlock {
            request_id: Uuid::new_v4(),
            id,
            watch: false,
        },
    )
    .await
}

pub async fn orphaned(socket: &mut Socket) -> Vec<Uuid> {
    match request(
        socket,
        ClientMessage::ListOrphanedBlocks {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    {
        ServerMessage::OrphanedBlocks { blocks, .. } => blocks,
        message => panic!("expected orphaned blocks, got {message:?}"),
    }
}

pub fn relationships(message: ServerMessage) -> (Option<Uuid>, Vec<Uuid>, Vec<Uuid>) {
    match message {
        ServerMessage::ReadBlock {
            parent,
            references,
            backrefs,
            ..
        } => (parent, references, backrefs),
        message => panic!("expected block read, got {message:?}"),
    }
}
