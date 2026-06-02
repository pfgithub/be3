use std::{
    collections::HashMap,
    env, fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

const DEFAULT_ADDR: &str = "127.0.0.1:9090";
const DEFAULT_DATA_DIR: &str = "block-data";

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let config = Config::from_env();
    let store = Arc::new(BlockStore::new(config.data_dir));
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

        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, store).await {
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

async fn handle_connection(stream: TcpStream, store: Arc<BlockStore>) -> Result<(), ServerError> {
    let mut socket = accept_async(stream).await?;

    while let Some(message) = socket.next().await {
        let message = message?;

        match message {
            Message::Text(text) => {
                let response = handle_text_message(&store, &text).await;
                socket
                    .send(Message::Text(serde_json::to_string(&response)?))
                    .await?;
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

    Ok(())
}

async fn handle_text_message(store: &BlockStore, text: &str) -> ServerMessage {
    let command = match serde_json::from_str::<ClientMessage>(text) {
        Ok(command) => command,
        Err(error) => {
            return ServerMessage::Error {
                command: None,
                id: None,
                code: ErrorCode::InvalidMessage,
                message: format!("invalid command JSON: {error}"),
            };
        }
    };

    match command {
        ClientMessage::CreateBlock {
            id,
            block_type,
            data,
        } => match store.create_block(id, block_type, data).await {
            Ok(()) => ServerMessage::Ok {
                command: CommandKind::CreateBlock,
                id,
                seq: None,
            },
            Err(error) => error.to_response(CommandKind::CreateBlock, id),
        },
        ClientMessage::UpdateBlock { id, seq, operation } => {
            match store.update_block(id, seq, operation).await {
                Ok(()) => ServerMessage::Ok {
                    command: CommandKind::UpdateBlock,
                    id,
                    seq: Some(seq),
                },
                Err(error) => error.to_response(CommandKind::UpdateBlock, id),
            }
        }
        ClientMessage::ReadBlock { id, offset, len } => {
            match store.read_block(id, offset, len).await {
                Ok(read) => ServerMessage::ReadBlock {
                    command: CommandKind::ReadBlock,
                    id,
                    data: read.data,
                    offset: read.offset,
                    len: read.len,
                    total_size: read.total_size,
                },
                Err(error) => error.to_response(CommandKind::ReadBlock, id),
            }
        }
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
    },
}

#[derive(Debug, Serialize)]
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
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CommandKind {
    CreateBlock,
    UpdateBlock,
    ReadBlock,
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
        assert_eq!(read.total_size, 5);

        let read = store.read_block(id, 3, 99).await.unwrap();
        assert_eq!(read.data, vec![4, 5]);
        assert_eq!(read.offset, 3);
        assert_eq!(read.len, 2);
        assert_eq!(read.total_size, 5);

        let read = store.read_block(id, 99, 10).await.unwrap();
        assert!(read.data.is_empty());
        assert_eq!(read.offset, 99);
        assert_eq!(read.len, 0);
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
        let server_store = Arc::clone(&store);
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, server_store).await.unwrap();
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
                    "data": [1, 2, 3]
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
                    "len": 10
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

    fn test_root() -> PathBuf {
        env::temp_dir().join(format!("block-server-test-{}", Uuid::new_v4()))
    }
}
