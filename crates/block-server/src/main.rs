use std::{
    collections::HashMap,
    env, fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

const DEFAULT_ADDR: &str = "127.0.0.1:9090";
const DEFAULT_DATA_DIR: &str = "block-data";

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let config = Config::from_env();
    let store = Arc::new(BlockStore::new(config.data_dir));
    let watch_hub = Arc::new(WatchHub::new());
    fs::create_dir_all(store.root()).await?;

    let listener = TcpListener::bind(&config.addr).await?;
    println!(
        "{} server listening on ws://{} storing blocks in {}",
        block::name(),
        config.addr,
        store.root().display()
    );

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let store = Arc::clone(&store);
        let watch_hub = Arc::clone(&watch_hub);

        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, store, watch_hub).await {
                eprintln!("connection {peer_addr} closed with error: {error}");
            }
        });
    }
}

struct Config {
    addr: String,
    data_dir: PathBuf,
}

impl Config {
    fn from_env() -> Self {
        let mut args = env::args().skip(1);

        Self {
            addr: args.next().unwrap_or_else(|| DEFAULT_ADDR.to_string()),
            data_dir: args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR)),
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    store: Arc<BlockStore>,
    watch_hub: Arc<WatchHub>,
) -> Result<(), ServerError> {
    let mut socket = accept_async(stream).await?;
    let (outbound, mut outbound_messages) = mpsc::unbounded_channel();
    let client_id = watch_hub.next_client_id();

    loop {
        tokio::select! {
            Some(outbound_message) = outbound_messages.recv() => {
                socket
                    .send(Message::Text(serde_json::to_string(&outbound_message)?))
                    .await?;
            }
            Some(message) = socket.next() => {
                let message = message?;

                match message {
                    Message::Text(text) => {
                        let outcome = handle_text_message(
                            &store,
                            &watch_hub,
                            client_id,
                            outbound.clone(),
                            &text,
                        )
                        .await;
                        socket
                            .send(Message::Text(serde_json::to_string(&outcome.response)?))
                            .await?;

                        for notification in outcome.notifications {
                            watch_hub.broadcast(notification.id(), notification).await;
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => socket.send(Message::Pong(payload)).await?,
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                        let response = ServerMessage::Error {
                            command: None,
                            id: None,
                            code: ErrorCode::UnsupportedMessage,
                            message: "only JSON text websocket messages are supported".to_string(),
                        };
                        socket
                            .send(Message::Text(serde_json::to_string(&response)?))
                            .await?;
                    }
                }
            }
            else => break,
        }
    }

    watch_hub.remove_client(client_id).await;

    Ok(())
}

async fn handle_text_message(
    store: &BlockStore,
    watch_hub: &WatchHub,
    client_id: ClientId,
    outbound: OutboundMessages,
    text: &str,
) -> MessageOutcome {
    let command = match serde_json::from_str::<ClientMessage>(text) {
        Ok(command) => command,
        Err(error) => {
            return MessageOutcome::single(ServerMessage::Error {
                command: None,
                id: None,
                code: ErrorCode::InvalidMessage,
                message: format!("invalid command JSON: {error}"),
            });
        }
    };

    match command {
        ClientMessage::CreateBlock {
            id,
            block_type,
            data,
            watch,
        } => match store.create_block(id, block_type, data).await {
            Ok(()) => {
                if watch {
                    watch_hub.watch(id, client_id, outbound).await;
                }

                ServerMessage::Ok {
                    command: CommandKind::CreateBlock,
                    id,
                    seq: None,
                }
                .into()
            }
            Err(error) => error.to_response(CommandKind::CreateBlock, id).into(),
        },
        ClientMessage::UpdateBlock { id, seq, operation } => {
            match store.update_block(id, seq, operation.clone()).await {
                Ok(()) => MessageOutcome {
                    response: ServerMessage::Ok {
                        command: CommandKind::UpdateBlock,
                        id,
                        seq: Some(seq),
                    },
                    notifications: vec![ServerMessage::BlockUpdated { id, seq, operation }],
                },
                Err(error) => error.to_response(CommandKind::UpdateBlock, id).into(),
            }
        }
        ClientMessage::ReadBlock {
            id,
            offset,
            len,
            watch,
        } => match store.read_block(id, offset, len).await {
            Ok(read) => {
                if watch {
                    watch_hub.watch(id, client_id, outbound).await;
                }

                ServerMessage::ReadBlock {
                    command: CommandKind::ReadBlock,
                    id,
                    data: read.data,
                    offset: read.offset,
                    len: read.len,
                    seq: read.seq,
                    total_size: read.total_size,
                }
                .into()
            }
            Err(error) => error.to_response(CommandKind::ReadBlock, id).into(),
        },
        ClientMessage::UnwatchBlock { id } => {
            watch_hub.unwatch(id, client_id).await;
            ServerMessage::Ok {
                command: CommandKind::UnwatchBlock,
                id,
                seq: None,
            }
            .into()
        }
        ClientMessage::PostPresence { id, data } => MessageOutcome {
            response: ServerMessage::Ok {
                command: CommandKind::PostPresence,
                id,
                seq: None,
            },
            notifications: vec![ServerMessage::Presence { id, data }],
        },
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum ClientMessage {
    CreateBlock {
        id: Uuid,
        #[serde(rename = "type")]
        block_type: Uuid,
        data: Vec<u8>,
        watch: bool,
    },
    UpdateBlock {
        id: Uuid,
        seq: u64,
        operation: Vec<u8>,
    },
    ReadBlock {
        id: Uuid,
        offset: u64,
        len: u64,
        watch: bool,
    },
    UnwatchBlock {
        id: Uuid,
    },
    PostPresence {
        id: Uuid,
        data: Vec<u8>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ServerMessage {
    Ok {
        command: CommandKind,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },
    #[serde(rename = "ok")]
    ReadBlock {
        command: CommandKind,
        id: Uuid,
        data: Vec<u8>,
        offset: u64,
        len: u64,
        seq: u64,
        total_size: u64,
    },
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<CommandKind>,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<Uuid>,
        code: ErrorCode,
        message: String,
    },
    BlockUpdated {
        id: Uuid,
        seq: u64,
        operation: Vec<u8>,
    },
    Presence {
        id: Uuid,
        data: Vec<u8>,
    },
}

impl ServerMessage {
    fn id(&self) -> Uuid {
        match self {
            Self::Ok { id, .. }
            | Self::ReadBlock { id, .. }
            | Self::Error { id: Some(id), .. }
            | Self::BlockUpdated { id, .. }
            | Self::Presence { id, .. } => *id,
            Self::Error { id: None, .. } => unreachable!("broadcast messages always have ids"),
        }
    }
}

impl From<ServerMessage> for MessageOutcome {
    fn from(response: ServerMessage) -> Self {
        Self {
            response,
            notifications: Vec::new(),
        }
    }
}

struct MessageOutcome {
    response: ServerMessage,
    notifications: Vec<ServerMessage>,
}

impl MessageOutcome {
    fn single(response: ServerMessage) -> Self {
        response.into()
    }
}

type ClientId = u64;
type OutboundMessages = mpsc::UnboundedSender<ServerMessage>;

struct WatchHub {
    next_client_id: AtomicU64,
    watchers: Mutex<HashMap<Uuid, HashMap<ClientId, OutboundMessages>>>,
}

impl WatchHub {
    fn new() -> Self {
        Self {
            next_client_id: AtomicU64::new(1),
            watchers: Mutex::new(HashMap::new()),
        }
    }

    fn next_client_id(&self) -> ClientId {
        self.next_client_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn watch(&self, id: Uuid, client_id: ClientId, outbound: OutboundMessages) {
        self.watchers
            .lock()
            .await
            .entry(id)
            .or_default()
            .insert(client_id, outbound);
    }

    async fn unwatch(&self, id: Uuid, client_id: ClientId) {
        let mut watchers = self.watchers.lock().await;
        let Some(block_watchers) = watchers.get_mut(&id) else {
            return;
        };

        block_watchers.remove(&client_id);

        if block_watchers.is_empty() {
            watchers.remove(&id);
        }
    }

    async fn remove_client(&self, client_id: ClientId) {
        let mut watchers = self.watchers.lock().await;
        let mut empty_blocks = Vec::new();

        for (id, block_watchers) in watchers.iter_mut() {
            block_watchers.remove(&client_id);

            if block_watchers.is_empty() {
                empty_blocks.push(*id);
            }
        }

        for id in empty_blocks {
            watchers.remove(&id);
        }
    }

    async fn broadcast(&self, id: Uuid, message: ServerMessage) {
        let watchers = self.watchers.lock().await;
        let Some(block_watchers) = watchers.get(&id) else {
            return;
        };

        for outbound in block_watchers.values() {
            let _ = outbound.send(message.clone());
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CommandKind {
    CreateBlock,
    UpdateBlock,
    ReadBlock,
    UnwatchBlock,
    PostPresence,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    BlockAlreadyExists,
    BlockNotFound,
    InvalidMessage,
    InvalidSeq,
    StorageError,
    UnsupportedMessage,
}

struct BlockStore {
    root: PathBuf,
    locks: Mutex<HashMap<Uuid, Arc<Mutex<()>>>>,
}

impl BlockStore {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    async fn create_block(
        &self,
        id: Uuid,
        block_type: Uuid,
        data: Vec<u8>,
    ) -> Result<(), StoreError> {
        let block_path = self.block_path(id);
        match fs::create_dir(&block_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(StoreError::BlockAlreadyExists);
            }
            Err(error) => return Err(StoreError::Io(error)),
        }

        let snapshots_path = block_path.join("snapshots");
        let operations_path = block_path.join("operations");
        fs::create_dir(&snapshots_path).await?;
        fs::create_dir(&operations_path).await?;

        let info = BlockInfo { block_type };
        let info_bytes = serde_json::to_vec_pretty(&info)?;
        fs::write(block_path.join("info.json"), info_bytes).await?;
        fs::write(snapshots_path.join("0"), data).await?;

        Ok(())
    }

    async fn update_block(&self, id: Uuid, seq: u64, operation: Vec<u8>) -> Result<(), StoreError> {
        if seq == 0 {
            return Err(StoreError::InvalidSeq {
                expected: 1,
                actual: seq,
            });
        }

        let lock = self.lock_for(id).await;
        let _guard = lock.lock().await;

        let block_path = self.block_path(id);
        if !block_path.is_dir() {
            return Err(StoreError::BlockNotFound);
        }

        let operations_path = block_path.join("operations");
        let expected = next_operation_seq(&operations_path).await?;

        if seq != expected {
            return Err(StoreError::InvalidSeq {
                expected,
                actual: seq,
            });
        }

        let operation_path = operations_path.join(seq.to_string());
        write_new_file(operation_path, operation)
            .await
            .map_err(|error| match error {
                NewFileError::AlreadyExists => StoreError::InvalidSeq {
                    expected,
                    actual: seq,
                },
                NewFileError::Io(error) => StoreError::Io(error),
            })?;

        Ok(())
    }

    async fn read_block(&self, id: Uuid, offset: u64, len: u64) -> Result<BlockRead, StoreError> {
        let snapshot_path = self.block_path(id).join("snapshots").join("0");
        let data = match fs::read(snapshot_path).await {
            Ok(data) => data,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(StoreError::BlockNotFound);
            }
            Err(error) => return Err(StoreError::Io(error)),
        };

        let total_size = data.len() as u64;
        let start = offset.min(total_size) as usize;
        let requested_end = offset.saturating_add(len).min(total_size);
        let end = requested_end.max(offset.min(total_size)) as usize;
        let data = data[start..end].to_vec();

        Ok(BlockRead {
            offset,
            len: data.len() as u64,
            seq: 0,
            total_size,
            data,
        })
    }

    fn block_path(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
    }

    async fn lock_for(&self, id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        Arc::clone(locks.entry(id).or_insert_with(|| Arc::new(Mutex::new(()))))
    }
}

struct BlockRead {
    data: Vec<u8>,
    offset: u64,
    len: u64,
    seq: u64,
    total_size: u64,
}

#[derive(Serialize)]
struct BlockInfo {
    #[serde(rename = "type")]
    block_type: Uuid,
}

async fn next_operation_seq(operations_path: &Path) -> Result<u64, StoreError> {
    let mut highest_seq = 0;
    let mut entries = fs::read_dir(operations_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };

        let Ok(seq) = file_name.parse::<u64>() else {
            continue;
        };

        highest_seq = highest_seq.max(seq);
    }

    Ok(highest_seq + 1)
}

async fn write_new_file(path: PathBuf, data: Vec<u8>) -> Result<(), NewFileError> {
    use tokio::io::AsyncWriteExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                NewFileError::AlreadyExists
            } else {
                NewFileError::Io(error)
            }
        })?;

    file.write_all(&data).await.map_err(NewFileError::Io)?;
    file.flush().await.map_err(NewFileError::Io)?;

    Ok(())
}

enum NewFileError {
    AlreadyExists,
    Io(std::io::Error),
}

#[derive(Debug)]
enum StoreError {
    BlockAlreadyExists,
    BlockNotFound,
    InvalidSeq { expected: u64, actual: u64 },
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl StoreError {
    fn to_response(&self, command: CommandKind, id: Uuid) -> ServerMessage {
        ServerMessage::Error {
            command: Some(command),
            id: Some(id),
            code: self.code(),
            message: self.to_string(),
        }
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::BlockAlreadyExists => ErrorCode::BlockAlreadyExists,
            Self::BlockNotFound => ErrorCode::BlockNotFound,
            Self::InvalidSeq { .. } => ErrorCode::InvalidSeq,
            Self::Io(_) | Self::Json(_) => ErrorCode::StorageError,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockAlreadyExists => write!(formatter, "block already exists"),
            Self::BlockNotFound => write!(formatter, "block does not exist"),
            Self::InvalidSeq { expected, actual } => {
                write!(formatter, "invalid seq {actual}; expected {expected}")
            }
            Self::Io(error) => write!(formatter, "storage I/O error: {error}"),
            Self::Json(error) => write!(formatter, "storage JSON error: {error}"),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Debug)]
enum ServerError {
    Io(std::io::Error),
    Json(serde_json::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::WebSocket(error) => write!(formatter, "websocket error: {error}"),
        }
    }
}

impl From<std::io::Error> for ServerError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ServerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ServerError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    #[tokio::test]
    async fn create_block_writes_expected_files() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();

        let id = Uuid::new_v4();
        let block_type = Uuid::new_v4();
        store
            .create_block(id, block_type, vec![1, 2, 3])
            .await
            .unwrap();

        assert_eq!(
            fs::read(root.join(id.to_string()).join("snapshots").join("0"))
                .await
                .unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            fs::read_to_string(root.join(id.to_string()).join("info.json"))
                .await
                .unwrap(),
            format!("{{\n  \"type\": \"{block_type}\"\n}}")
        );

        assert!(matches!(
            store.create_block(id, block_type, vec![]).await,
            Err(StoreError::BlockAlreadyExists)
        ));

        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn update_block_requires_next_seq() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();

        let id = Uuid::new_v4();
        store
            .create_block(id, Uuid::new_v4(), vec![1])
            .await
            .unwrap();

        assert!(matches!(
            store.update_block(id, 2, vec![9]).await,
            Err(StoreError::InvalidSeq {
                expected: 1,
                actual: 2
            })
        ));

        store.update_block(id, 1, vec![9]).await.unwrap();
        assert_eq!(
            fs::read(root.join(id.to_string()).join("operations").join("1"))
                .await
                .unwrap(),
            vec![9]
        );

        assert!(matches!(
            store.update_block(id, 1, vec![10]).await,
            Err(StoreError::InvalidSeq {
                expected: 2,
                actual: 1
            })
        ));

        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn read_block_returns_only_requested_in_range_data() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();

        let id = Uuid::new_v4();
        store
            .create_block(id, Uuid::new_v4(), vec![1, 2, 3, 4, 5])
            .await
            .unwrap();

        let read = store.read_block(id, 1, 3).await.unwrap();
        assert_eq!(read.data, vec![2, 3, 4]);
        assert_eq!(read.offset, 1);
        assert_eq!(read.len, 3);
        assert_eq!(read.seq, 0);
        assert_eq!(read.total_size, 5);

        let read = store.read_block(id, 3, 99).await.unwrap();
        assert_eq!(read.data, vec![4, 5]);
        assert_eq!(read.offset, 3);
        assert_eq!(read.len, 2);
        assert_eq!(read.seq, 0);
        assert_eq!(read.total_size, 5);

        let read = store.read_block(id, 99, 10).await.unwrap();
        assert!(read.data.is_empty());
        assert_eq!(read.offset, 99);
        assert_eq!(read.len, 0);
        assert_eq!(read.seq, 0);
        assert_eq!(read.total_size, 5);

        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn running_server_accepts_json_messages() {
        let root = test_root();
        let store = Arc::new(BlockStore::new(root.clone()));
        fs::create_dir_all(store.root()).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let watch_hub = Arc::new(WatchHub::new());
        let server_store = Arc::clone(&store);
        let server_watch_hub = Arc::clone(&watch_hub);
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, server_store, server_watch_hub)
                .await
                .unwrap();
        });

        let (mut client, _) = connect_async(format!("ws://{addr}")).await.unwrap();

        let id = Uuid::new_v4();
        let block_type = Uuid::new_v4();
        client
            .send(Message::Text(
                serde_json::json!({
                    "command": "create_block",
                    "id": id,
                    "type": block_type,
                    "data": [1, 2, 3],
                    "watch": false
                })
                .to_string(),
            ))
            .await
            .unwrap();

        let response = client.next().await.unwrap().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response.into_text().unwrap()).unwrap(),
            serde_json::json!({
                "status": "ok",
                "command": "create_block",
                "id": id
            }),
        );

        client
            .send(Message::Text(
                serde_json::json!({
                    "command": "update_block",
                    "id": id,
                    "seq": 1,
                    "operation": [4, 5]
                })
                .to_string(),
            ))
            .await
            .unwrap();

        let response = client.next().await.unwrap().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response.into_text().unwrap()).unwrap(),
            serde_json::json!({
                "status": "ok",
                "command": "update_block",
                "id": id,
                "seq": 1
            }),
        );

        client
            .send(Message::Text(
                serde_json::json!({
                    "command": "read_block",
                    "id": id,
                    "offset": 1,
                    "len": 10,
                    "watch": false
                })
                .to_string(),
            ))
            .await
            .unwrap();

        let response = client.next().await.unwrap().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response.into_text().unwrap()).unwrap(),
            serde_json::json!({
                "status": "ok",
                "command": "read_block",
                "id": id,
                "data": [2, 3],
                "offset": 1,
                "len": 2,
                "seq": 0,
                "total_size": 3
            }),
        );

        client.close(None).await.unwrap();
        server.await.unwrap();

        assert_eq!(
            fs::read(root.join(id.to_string()).join("operations").join("1"))
                .await
                .unwrap(),
            vec![4, 5]
        );

        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn watchers_receive_updates_and_presence_until_unwatched() {
        let root = test_root();
        let store = Arc::new(BlockStore::new(root.clone()));
        let watch_hub = Arc::new(WatchHub::new());
        fs::create_dir_all(store.root()).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_store = Arc::clone(&store);
        let server_watch_hub = Arc::clone(&watch_hub);
        let server = tokio::spawn(async move {
            let mut connections = Vec::new();

            for _ in 0..3 {
                let (stream, _) = listener.accept().await.unwrap();
                let store = Arc::clone(&server_store);
                let watch_hub = Arc::clone(&server_watch_hub);
                connections.push(tokio::spawn(async move {
                    handle_connection(stream, store, watch_hub).await.unwrap();
                }));
            }

            for connection in connections {
                connection.await.unwrap();
            }
        });

        let (mut watcher_a, _) = connect_async(format!("ws://{addr}")).await.unwrap();
        let (mut watcher_b, _) = connect_async(format!("ws://{addr}")).await.unwrap();
        let (mut writer, _) = connect_async(format!("ws://{addr}")).await.unwrap();

        let id = Uuid::new_v4();
        let block_type = Uuid::new_v4();

        watcher_a
            .send(Message::Text(
                serde_json::json!({
                    "command": "create_block",
                    "id": id,
                    "type": block_type,
                    "data": [1, 2, 3],
                    "watch": true
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut watcher_a).await,
            serde_json::json!({
                "status": "ok",
                "command": "create_block",
                "id": id
            })
        );

        watcher_b
            .send(Message::Text(
                serde_json::json!({
                    "command": "read_block",
                    "id": id,
                    "offset": 0,
                    "len": 10,
                    "watch": true
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut watcher_b).await,
            serde_json::json!({
                "status": "ok",
                "command": "read_block",
                "id": id,
                "data": [1, 2, 3],
                "offset": 0,
                "len": 3,
                "seq": 0,
                "total_size": 3
            })
        );

        writer
            .send(Message::Text(
                serde_json::json!({
                    "command": "update_block",
                    "id": id,
                    "seq": 1,
                    "operation": [4, 5]
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut writer).await,
            serde_json::json!({
                "status": "ok",
                "command": "update_block",
                "id": id,
                "seq": 1
            })
        );

        let update = serde_json::json!({
            "status": "block_updated",
            "id": id,
            "seq": 1,
            "operation": [4, 5]
        });
        assert_eq!(next_json(&mut watcher_a).await, update);
        assert_eq!(next_json(&mut watcher_b).await, update);

        watcher_b
            .send(Message::Text(
                serde_json::json!({
                    "command": "unwatch_block",
                    "id": id
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut watcher_b).await,
            serde_json::json!({
                "status": "ok",
                "command": "unwatch_block",
                "id": id
            })
        );

        writer
            .send(Message::Text(
                serde_json::json!({
                    "command": "post_presence",
                    "id": id,
                    "data": [9, 8, 7]
                })
                .to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(
            next_json(&mut writer).await,
            serde_json::json!({
                "status": "ok",
                "command": "post_presence",
                "id": id
            })
        );
        assert_eq!(
            next_json(&mut watcher_a).await,
            serde_json::json!({
                "status": "presence",
                "id": id,
                "data": [9, 8, 7]
            })
        );
        assert_no_message(&mut watcher_b).await;

        watcher_a.close(None).await.unwrap();
        watcher_b.close(None).await.unwrap();
        writer.close(None).await.unwrap();
        server.await.unwrap();

        assert_eq!(
            fs::read(root.join(id.to_string()).join("operations").join("1"))
                .await
                .unwrap(),
            vec![4, 5]
        );
        assert!(!root.join(id.to_string()).join("presence").exists());

        fs::remove_dir_all(root).await.unwrap();
    }

    async fn next_json<S>(client: &mut S) -> serde_json::Value
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let message = client.next().await.unwrap().unwrap();
        serde_json::from_str(&message.into_text().unwrap()).unwrap()
    }

    async fn assert_no_message<S>(client: &mut S)
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(50), client.next()).await;
        assert!(result.is_err(), "client unexpectedly received a message");
    }

    fn test_root() -> PathBuf {
        env::temp_dir().join(format!("block-server-test-{}", Uuid::new_v4()))
    }
}
