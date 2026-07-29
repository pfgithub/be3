use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use block::{
    BlockOperation, BlockParent, BlockReference, BlockReferenceList, ClientMessage, CommandKind,
    ErrorCode, OperationRecord, ReferenceDelta, ServerMessage, MAX_NAME_BYTES,
};
use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        http::StatusCode,
        Message,
    },
};
use uuid::Uuid;

const ACCOUNT_HEADER: &str = "x-block-account-id";

pub async fn serve(listener: TcpListener, data_dir: impl Into<PathBuf>) -> Result<(), ServerError> {
    let root = data_dir.into();
    fs::create_dir_all(&root).await?;
    let store = Arc::new(BlockStore::open(root).await?);
    let watch_hub = Arc::new(WatchHub::new());

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

async fn handle_connection(
    stream: TcpStream,
    store: Arc<BlockStore>,
    watch_hub: Arc<WatchHub>,
) -> Result<(), ServerError> {
    let mut account_id = None;
    let socket = accept_hdr_async(
        stream,
        |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            let parsed = request
                .headers()
                .get(ACCOUNT_HEADER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| Uuid::parse_str(value).ok());
            let Some(parsed) = parsed else {
                return Err(http_error(
                    StatusCode::BAD_REQUEST,
                    format!("missing or invalid {ACCOUNT_HEADER} header"),
                ));
            };
            account_id = Some(parsed);
            Ok(response)
        },
    )
    .await?;
    let account_id = account_id.expect("accepted handshake omitted account identity");
    let (mut sink, mut source) = socket.split();
    let (outbound, mut outbound_rx) = mpsc::unbounded_channel();
    let client_id = watch_hub.next_client_id();

    loop {
        tokio::select! {
            Some(message) = outbound_rx.recv() => {
                sink.send(Message::Text(serde_json::to_string(&message)?)).await?;
            }
            Some(message) = source.next() => {
                match message? {
                    Message::Text(text) => {
                        let (response, notification) =
                            handle_text_message(
                                &store,
                                &watch_hub,
                                client_id,
                                account_id,
                                outbound.clone(),
                                &text,
                            ).await;
                        sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                        if let Some(notification) = notification {
                            match notification {
                                ServerMessage::BatchUpdated { operations } => {
                                    watch_hub.broadcast_batch(operations).await;
                                }
                                notification => watch_hub.broadcast(notification).await,
                            }
                        }
                        watch_hub.broadcast_reference_lists(&store).await;
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => sink.send(Message::Pong(payload)).await?,
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                        let response = ServerMessage::Error {
                            request_id: None,
                            command: None,
                            id: None,
                            code: ErrorCode::UnsupportedMessage,
                            message: "only JSON text websocket messages are supported".into(),
                            expected_seq: None,
                        };
                        sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                    }
                }
            }
            else => break,
        }
    }

    watch_hub.remove_client(client_id).await;
    Ok(())
}

fn http_error(status: StatusCode, message: String) -> ErrorResponse {
    let mut response = ErrorResponse::new(Some(message));
    *response.status_mut() = status;
    response
}

async fn handle_text_message(
    store: &BlockStore,
    watch_hub: &WatchHub,
    client_id: ClientId,
    account_id: Uuid,
    outbound: OutboundMessages,
    text: &str,
) -> (ServerMessage, Option<ServerMessage>) {
    let command = match serde_json::from_str::<ClientMessage>(text) {
        Ok(command) => command,
        Err(error) => {
            return (
                ServerMessage::Error {
                    request_id: None,
                    command: None,
                    id: None,
                    code: ErrorCode::InvalidMessage,
                    message: format!("invalid command JSON: {error}"),
                    expected_seq: None,
                },
                None,
            );
        }
    };

    match command {
        ClientMessage::CreateBlock {
            request_id,
            id,
            block_type,
            data,
            implicit_name,
            references,
            watch,
        } => {
            let lock = store.lock_for(id).await;
            let _guard = lock.lock().await;
            let response = match store
                .create_block_unlocked(id, block_type, account_id, data, implicit_name, references)
                .await
            {
                Ok(()) => {
                    if watch {
                        watch_hub.watch(id, client_id, outbound).await;
                    }
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::CreateBlock,
                        id,
                        seq: None,
                        operation_id: None,
                    }
                }
                Err(error) => error.to_response(request_id, CommandKind::CreateBlock, id),
            };
            (response, None)
        }
        ClientMessage::UpdateBlock {
            request_id,
            id,
            seq,
            operation_id,
            operation,
            implicit_name,
            references,
        } => {
            let lock = store.lock_for(id).await;
            let _guard = lock.lock().await;
            match store
                .update_block_unlocked(
                    id,
                    seq,
                    operation_id,
                    account_id,
                    operation,
                    implicit_name,
                    references,
                )
                .await
            {
                Ok(UpdateOutcome::Inserted(record, name)) => (
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::UpdateBlock,
                        id,
                        seq: Some(record.seq),
                        operation_id: Some(record.operation_id),
                    },
                    Some(ServerMessage::BlockUpdated {
                        id,
                        name,
                        operation: record,
                    }),
                ),
                Ok(UpdateOutcome::Duplicate(record, _)) => (
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::UpdateBlock,
                        id,
                        seq: Some(record.seq),
                        operation_id: Some(record.operation_id),
                    },
                    None,
                ),
                Err(error) => (
                    error.to_response(request_id, CommandKind::UpdateBlock, id),
                    None,
                ),
            }
        }
        ClientMessage::UpdateBatch {
            request_id,
            updates,
        } => {
            if updates.is_empty() {
                return (
                    ServerMessage::Error {
                        request_id: Some(request_id),
                        command: Some(CommandKind::UpdateBatch),
                        id: None,
                        code: ErrorCode::InvalidMessage,
                        message: "update batch must not be empty".into(),
                        expected_seq: None,
                    },
                    None,
                );
            }
            let ids: BTreeSet<_> = updates.iter().map(|update| update.id).collect();
            if ids.len() != updates.len() {
                return (
                    ServerMessage::Error {
                        request_id: Some(request_id),
                        command: Some(CommandKind::UpdateBatch),
                        id: None,
                        code: ErrorCode::InvalidMessage,
                        message: "update batch may contain at most one update per block".into(),
                        expected_seq: None,
                    },
                    None,
                );
            }
            let mut locks = Vec::with_capacity(ids.len());
            for id in ids {
                locks.push(store.lock_for(id).await);
            }
            let mut guards = Vec::with_capacity(locks.len());
            for lock in &locks {
                guards.push(lock.lock().await);
            }

            let mut operations = Vec::with_capacity(updates.len());
            let mut inserted = Vec::new();
            for update in updates {
                match store
                    .update_block_unlocked(
                        update.id,
                        update.seq,
                        update.operation_id,
                        account_id,
                        update.operation,
                        update.implicit_name,
                        update.references,
                    )
                    .await
                {
                    Ok(UpdateOutcome::Inserted(operation, name)) => {
                        let operation = BlockOperation {
                            id: update.id,
                            name,
                            operation,
                        };
                        inserted.push(operation.clone());
                        operations.push(operation);
                    }
                    Ok(UpdateOutcome::Duplicate(operation, name)) => {
                        operations.push(BlockOperation {
                            id: update.id,
                            name,
                            operation,
                        });
                    }
                    Err(error) => {
                        return (
                            error.to_response(request_id, CommandKind::UpdateBatch, update.id),
                            None,
                        );
                    }
                }
            }
            (
                ServerMessage::BatchOk {
                    request_id,
                    command: CommandKind::UpdateBatch,
                    operations,
                },
                (!inserted.is_empty()).then_some(ServerMessage::BatchUpdated {
                    operations: inserted,
                }),
            )
        }
        ClientMessage::ReadBlock {
            request_id,
            id,
            watch,
        } => {
            let lock = store.lock_for(id).await;
            let _guard = lock.lock().await;
            let response = match store.read_block_unlocked(id).await {
                Ok(read) => {
                    if watch {
                        watch_hub.watch(id, client_id, outbound).await;
                    }
                    ServerMessage::ReadBlock {
                        request_id,
                        command: CommandKind::ReadBlock,
                        id,
                        block_type: read.block_type,
                        author: read.author,
                        snapshot: read.snapshot,
                        snapshot_seq: read.snapshot_seq,
                        operations: read.operations,
                        parent: read.parent,
                        name: read.name,
                    }
                }
                Err(error) => error.to_response(request_id, CommandKind::ReadBlock, id),
            };
            (response, None)
        }
        ClientMessage::UnwatchBlock { request_id, id } => {
            watch_hub.unwatch(id, client_id).await;
            (
                ServerMessage::Ok {
                    request_id,
                    command: CommandKind::UnwatchBlock,
                    id,
                    seq: None,
                    operation_id: None,
                },
                None,
            )
        }
        ClientMessage::PostPresence {
            request_id,
            id,
            data,
        } => (
            ServerMessage::Ok {
                request_id,
                command: CommandKind::PostPresence,
                id,
                seq: None,
                operation_id: None,
            },
            Some(ServerMessage::Presence { id, data }),
        ),
        ClientMessage::SetBlockParent {
            request_id,
            id,
            parent,
        } => {
            let lock = store.lock_for(id).await;
            let _guard = lock.lock().await;
            let response = match store.set_parent_unlocked(id, parent).await {
                Ok(()) => ServerMessage::Ok {
                    request_id,
                    command: CommandKind::SetBlockParent,
                    id,
                    seq: None,
                    operation_id: None,
                },
                Err(error) => error.to_response(request_id, CommandKind::SetBlockParent, id),
            };
            (response, None)
        }
        ClientMessage::SetBlockName {
            request_id,
            id,
            name,
        } => {
            let lock = store.lock_for(id).await;
            let _guard = lock.lock().await;
            match store.set_name_unlocked(id, name).await {
                Ok(name) => (
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::SetBlockName,
                        id,
                        seq: None,
                        operation_id: None,
                    },
                    Some(ServerMessage::BlockNameUpdated { id, name }),
                ),
                Err(error) => (
                    error.to_response(request_id, CommandKind::SetBlockName, id),
                    None,
                ),
            }
        }
        ClientMessage::ListReferences {
            request_id,
            list,
            watch,
        } => {
            let blocks = store.references(list).await;
            if watch {
                watch_hub
                    .watch_references(list, client_id, outbound, blocks.clone())
                    .await;
            }
            (
                ServerMessage::References {
                    request_id,
                    list,
                    blocks,
                },
                None,
            )
        }
        ClientMessage::UnwatchReferences { request_id, list } => {
            watch_hub.unwatch_references(list, client_id).await;
            (
                ServerMessage::Ok {
                    request_id,
                    command: CommandKind::UnwatchReferences,
                    id: Uuid::nil(),
                    seq: None,
                    operation_id: None,
                },
                None,
            )
        }
    }
}

type ClientId = u64;
type OutboundMessages = mpsc::UnboundedSender<ServerMessage>;

struct WatchHub {
    next_client_id: AtomicU64,
    watchers: Mutex<HashMap<Uuid, HashMap<ClientId, OutboundMessages>>>,
    reference_watchers: Mutex<HashMap<BlockReferenceList, HashMap<ClientId, ReferenceWatch>>>,
}

struct ReferenceWatch {
    outbound: OutboundMessages,
    last: Vec<BlockReference>,
}

impl WatchHub {
    fn new() -> Self {
        Self {
            next_client_id: AtomicU64::new(1),
            watchers: Mutex::new(HashMap::new()),
            reference_watchers: Mutex::new(HashMap::new()),
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
        if let Some(entries) = watchers.get_mut(&id) {
            entries.remove(&client_id);
            if entries.is_empty() {
                watchers.remove(&id);
            }
        }
    }

    async fn remove_client(&self, client_id: ClientId) {
        let mut watchers = self.watchers.lock().await;
        watchers.retain(|_, entries| {
            entries.remove(&client_id);
            !entries.is_empty()
        });
        let mut watchers = self.reference_watchers.lock().await;
        watchers.retain(|_, entries| {
            entries.remove(&client_id);
            !entries.is_empty()
        });
    }

    async fn watch_references(
        &self,
        list: BlockReferenceList,
        client_id: ClientId,
        outbound: OutboundMessages,
        last: Vec<BlockReference>,
    ) {
        self.reference_watchers
            .lock()
            .await
            .entry(list)
            .or_default()
            .insert(client_id, ReferenceWatch { outbound, last });
    }

    async fn unwatch_references(&self, list: BlockReferenceList, client_id: ClientId) {
        let mut watchers = self.reference_watchers.lock().await;
        if let Some(entries) = watchers.get_mut(&list) {
            entries.remove(&client_id);
            if entries.is_empty() {
                watchers.remove(&list);
            }
        }
    }

    async fn broadcast_reference_lists(&self, store: &BlockStore) {
        let lists: Vec<_> = self
            .reference_watchers
            .lock()
            .await
            .keys()
            .copied()
            .collect();
        for list in lists {
            let blocks = store.references(list).await;
            let mut watchers = self.reference_watchers.lock().await;
            let Some(entries) = watchers.get_mut(&list) else {
                continue;
            };
            for watch in entries.values_mut() {
                if watch.last != blocks {
                    watch.last.clone_from(&blocks);
                    let _ = watch.outbound.send(ServerMessage::ReferencesUpdated {
                        list,
                        blocks: blocks.clone(),
                    });
                }
            }
        }
    }

    async fn broadcast(&self, message: ServerMessage) {
        let Some(id) = message.id() else {
            return;
        };
        let watchers = self.watchers.lock().await;
        if let Some(entries) = watchers.get(&id) {
            for outbound in entries.values() {
                let _ = outbound.send(message.clone());
            }
        }
    }

    async fn broadcast_batch(&self, operations: Vec<BlockOperation>) {
        let watchers = self.watchers.lock().await;
        let mut deliveries: HashMap<ClientId, (OutboundMessages, Vec<BlockOperation>)> =
            HashMap::new();
        for operation in operations {
            if let Some(entries) = watchers.get(&operation.id) {
                for (&client_id, outbound) in entries {
                    let delivery = deliveries
                        .entry(client_id)
                        .or_insert_with(|| (outbound.clone(), Vec::new()));
                    delivery.1.push(operation.clone());
                }
            }
        }
        for (_, (outbound, operations)) in deliveries {
            let _ = outbound.send(ServerMessage::BatchUpdated { operations });
        }
    }
}

struct BlockStore {
    root: PathBuf,
    locks: Mutex<HashMap<Uuid, Arc<Mutex<()>>>>,
    dependencies: Mutex<DependencyState>,
}

impl BlockStore {
    #[cfg(test)]
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            locks: Mutex::new(HashMap::new()),
            dependencies: Mutex::new(DependencyState::default()),
        }
    }

    async fn open(root: PathBuf) -> Result<Self, ServerError> {
        let path = root.join("dependencies.json");
        let dependencies = match fs::read(path).await {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(error) if error.kind() == ErrorKind::NotFound => DependencyState::default(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            root,
            locks: Mutex::new(HashMap::new()),
            dependencies: Mutex::new(dependencies),
        })
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    async fn lock_for(&self, id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        Arc::clone(locks.entry(id).or_insert_with(|| Arc::new(Mutex::new(()))))
    }

    async fn create_block_unlocked(
        &self,
        id: Uuid,
        block_type: Uuid,
        author: Uuid,
        data: Vec<u8>,
        implicit_name: String,
        references: Vec<Uuid>,
    ) -> Result<(), StoreError> {
        validate_name(&implicit_name)?;
        let references = normalize_ids(references);
        let mut dependencies = self.dependencies.lock().await;
        if dependencies.blocks.contains_key(&id) {
            return Err(StoreError::BlockAlreadyExists);
        }
        if references
            .iter()
            .any(|reference| *reference != id && !dependencies.blocks.contains_key(reference))
        {
            return Err(StoreError::ReferencedBlockNotFound);
        }
        let block_path = self.block_path(id);
        match fs::create_dir(&block_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(StoreError::BlockAlreadyExists);
            }
            Err(error) => return Err(error.into()),
        }
        fs::create_dir(block_path.join("snapshots")).await?;
        fs::create_dir(block_path.join("operations")).await?;
        fs::write(
            block_path.join("info.json"),
            serde_json::to_vec_pretty(&BlockInfo { block_type })?,
        )
        .await?;
        fs::write(block_path.join("snapshots").join("0"), data).await?;
        let mut updated = dependencies.clone();
        updated.blocks.insert(
            id,
            DependencyBlock {
                block_type,
                author,
                name: implicit_name,
                explicit_name: None,
                parent: BlockParent::Orphaned,
                references,
                backrefs: Vec::new(),
            },
        );
        let references = updated.blocks[&id].references.clone();
        for reference in references {
            let backrefs = &mut updated.blocks.get_mut(&reference).unwrap().backrefs;
            if !backrefs.contains(&id) {
                backrefs.push(id);
            }
        }
        if let Err(error) = self.persist_dependencies(&updated).await {
            let _ = fs::remove_dir_all(&block_path).await;
            return Err(error);
        }
        *dependencies = updated;
        Ok(())
    }

    async fn update_block_unlocked(
        &self,
        id: Uuid,
        seq: Option<u64>,
        operation_id: Uuid,
        author: Uuid,
        operation: Vec<u8>,
        implicit_name: String,
        references: ReferenceDelta,
    ) -> Result<UpdateOutcome, StoreError> {
        validate_name(&implicit_name)?;
        let operations_path = self.block_path(id).join("operations");
        if !operations_path.is_dir() {
            return Err(StoreError::BlockNotFound);
        }

        let records = read_operations(&operations_path).await?;
        if let Some(existing) = records
            .iter()
            .find(|record| record.operation_id == operation_id)
        {
            if existing.operation == operation && existing.references == references {
                let dependencies = self.dependencies.lock().await;
                let name = dependencies
                    .blocks
                    .get(&id)
                    .ok_or(StoreError::BlockNotFound)?
                    .name
                    .clone();
                return Ok(UpdateOutcome::Duplicate(existing.clone(), name));
            }
            return Err(StoreError::ConflictingOperationId);
        }

        let expected = records.last().map_or(1, |record| record.seq + 1);
        if seq.is_some_and(|seq| seq != expected) {
            return Err(StoreError::InvalidSeq {
                expected,
                actual: seq.unwrap(),
            });
        }

        let record = OperationRecord {
            seq: expected,
            operation_id,
            author,
            operation,
            references,
        };
        let path = operations_path.join(expected.to_string());
        write_new_file(path, serde_json::to_vec(&record)?).await?;
        let mut dependencies = self.dependencies.lock().await;
        let mut updated = dependencies.clone();
        if let Err(error) = updated.apply_references(id, &record.references) {
            let _ = fs::remove_file(operations_path.join(expected.to_string())).await;
            return Err(error);
        }
        let block = updated
            .blocks
            .get_mut(&id)
            .ok_or(StoreError::BlockNotFound)?;
        if block.explicit_name.is_none() {
            block.name = implicit_name;
        }
        let name = block.name.clone();
        if let Err(error) = self.persist_dependencies(&updated).await {
            let _ = fs::remove_file(operations_path.join(expected.to_string())).await;
            return Err(error);
        }
        *dependencies = updated;
        Ok(UpdateOutcome::Inserted(record, name))
    }

    async fn read_block_unlocked(&self, id: Uuid) -> Result<BlockRead, StoreError> {
        let block_path = self.block_path(id);
        let info: BlockInfo =
            serde_json::from_slice(&read_required(block_path.join("info.json")).await?)?;
        let snapshot_seq = highest_snapshot_seq(&block_path.join("snapshots")).await?;
        let snapshot =
            read_required(block_path.join("snapshots").join(snapshot_seq.to_string())).await?;
        let operations = read_operations(&block_path.join("operations"))
            .await?
            .into_iter()
            .filter(|record| record.seq > snapshot_seq)
            .collect();
        let dependencies = self.dependencies.lock().await;
        let dependency = dependencies
            .blocks
            .get(&id)
            .ok_or(StoreError::BlockNotFound)?;
        Ok(BlockRead {
            block_type: info.block_type,
            author: dependency.author,
            snapshot,
            snapshot_seq,
            operations,
            parent: dependency.parent,
            name: dependency.name.clone(),
        })
    }

    async fn set_parent_unlocked(&self, id: Uuid, parent: BlockParent) -> Result<(), StoreError> {
        let mut dependencies = self.dependencies.lock().await;
        let mut updated = dependencies.clone();
        updated.set_parent(id, parent)?;
        self.persist_dependencies(&updated).await?;
        *dependencies = updated;
        Ok(())
    }

    async fn set_name_unlocked(&self, id: Uuid, name: String) -> Result<String, StoreError> {
        validate_name(&name)?;
        let mut dependencies = self.dependencies.lock().await;
        let mut updated = dependencies.clone();
        let block = updated
            .blocks
            .get_mut(&id)
            .ok_or(StoreError::BlockNotFound)?;
        block.name.clone_from(&name);
        block.explicit_name = Some(name.clone());
        self.persist_dependencies(&updated).await?;
        *dependencies = updated;
        Ok(name)
    }

    async fn references(&self, list: BlockReferenceList) -> Vec<BlockReference> {
        let dependencies = self.dependencies.lock().await;
        let ids: Vec<_> = match list {
            BlockReferenceList::Orphans | BlockReferenceList::Roots => dependencies
                .blocks
                .iter()
                .filter_map(|(&id, block)| {
                    let parent = match list {
                        BlockReferenceList::Orphans => BlockParent::Orphaned,
                        BlockReferenceList::Roots => BlockParent::Root,
                        _ => unreachable!(),
                    };
                    (block.parent == parent).then_some(id)
                })
                .collect(),
            BlockReferenceList::References(id) => dependencies
                .blocks
                .get(&id)
                .map_or_else(Vec::new, |block| block.references.clone()),
            BlockReferenceList::Backrefs(id) => dependencies.backrefs(id),
            BlockReferenceList::Parents(id) => {
                let mut parents = Vec::new();
                let mut parent = dependencies.blocks.get(&id).map(|block| block.parent);
                while let Some(BlockParent::Uuid(parent_id)) = parent {
                    parents.push(parent_id);
                    parent = dependencies
                        .blocks
                        .get(&parent_id)
                        .map(|block| block.parent);
                }
                parents.reverse();
                parents
            }
        };
        ids.into_iter()
            .filter_map(|id| {
                dependencies.blocks.get(&id).map(|block| BlockReference {
                    id,
                    block_type: block.block_type,
                    author: block.author,
                    name: block.name.clone(),
                    parent: block.parent,
                    references: block.references.len(),
                })
            })
            .collect()
    }

    async fn persist_dependencies(&self, dependencies: &DependencyState) -> Result<(), StoreError> {
        fs::write(
            self.root.join("dependencies.json"),
            serde_json::to_vec_pretty(dependencies)?,
        )
        .await?;
        Ok(())
    }

    fn block_path(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
    }
}

enum UpdateOutcome {
    Inserted(OperationRecord, String),
    Duplicate(OperationRecord, String),
}

struct BlockRead {
    block_type: Uuid,
    author: Uuid,
    snapshot: Vec<u8>,
    snapshot_seq: u64,
    operations: Vec<OperationRecord>,
    parent: BlockParent,
    name: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct DependencyState {
    blocks: IndexMap<Uuid, DependencyBlock>,
}

impl DependencyState {
    fn apply_references(&mut self, id: Uuid, delta: &ReferenceDelta) -> Result<(), StoreError> {
        if delta
            .after
            .iter()
            .any(|reference| !self.blocks.contains_key(reference))
        {
            return Err(StoreError::ReferencedBlockNotFound);
        }
        if !self.blocks.contains_key(&id) {
            return Err(StoreError::BlockNotFound);
        }
        let before: HashSet<_> = delta.before.iter().copied().collect();
        let after: HashSet<_> = delta.after.iter().copied().collect();
        let previous = self.blocks[&id].references.clone();
        let mut references: Vec<_> = previous
            .iter()
            .copied()
            .filter(|reference| !before.contains(reference) || after.contains(reference))
            .filter(|reference| !after.contains(reference))
            .collect();
        references.extend(normalize_ids(delta.after.clone()));

        let previous: HashSet<_> = previous.into_iter().collect();
        let current: HashSet<_> = references.iter().copied().collect();
        for removed in previous.difference(&current) {
            self.blocks
                .get_mut(removed)
                .unwrap()
                .backrefs
                .retain(|backref| *backref != id);
        }
        for added in current.difference(&previous) {
            let backrefs = &mut self.blocks.get_mut(added).unwrap().backrefs;
            if !backrefs.contains(&id) {
                backrefs.push(id);
            }
        }
        self.blocks.get_mut(&id).unwrap().references = references;

        let still_referenced: HashSet<_> = self.blocks[&id].references.iter().copied().collect();
        for (&child_id, child) in &mut self.blocks {
            if child.parent == BlockParent::Uuid(id) && !still_referenced.contains(&child_id) {
                child.parent = BlockParent::Orphaned;
            }
        }
        Ok(())
    }

    fn set_parent(&mut self, id: Uuid, parent: BlockParent) -> Result<(), StoreError> {
        if !self.blocks.contains_key(&id) {
            return Err(StoreError::BlockNotFound);
        }
        let BlockParent::Uuid(parent_id) = parent else {
            self.blocks.get_mut(&id).unwrap().parent = parent;
            return Ok(());
        };
        let parent_block = self
            .blocks
            .get(&parent_id)
            .ok_or(StoreError::ReferencedBlockNotFound)?;
        if !parent_block.references.contains(&id) {
            return Err(StoreError::ParentMissingReference);
        }
        let mut ancestor = Some(parent_id);
        while let Some(current) = ancestor {
            if current == id {
                return Err(StoreError::ParentCycle);
            }
            ancestor = self
                .blocks
                .get(&current)
                .and_then(|block| match block.parent {
                    BlockParent::Uuid(parent) => Some(parent),
                    BlockParent::Orphaned | BlockParent::Root => None,
                });
        }
        self.blocks.get_mut(&id).unwrap().parent = parent;
        Ok(())
    }

    fn backrefs(&self, id: Uuid) -> Vec<Uuid> {
        self.blocks
            .get(&id)
            .map_or_else(Vec::new, |block| block.backrefs.clone())
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct DependencyBlock {
    block_type: Uuid,
    author: Uuid,
    name: String,
    explicit_name: Option<String>,
    parent: BlockParent,
    references: Vec<Uuid>,
    backrefs: Vec<Uuid>,
}

fn validate_name(name: &str) -> Result<(), StoreError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(StoreError::InvalidMessage(format!(
            "block name exceeds {MAX_NAME_BYTES} bytes"
        )));
    }
    Ok(())
}

fn normalize_ids(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

#[derive(Deserialize, Serialize)]
struct BlockInfo {
    #[serde(rename = "type")]
    block_type: Uuid,
}

async fn read_required(path: PathBuf) -> Result<Vec<u8>, StoreError> {
    fs::read(path).await.map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::BlockNotFound
        } else {
            StoreError::Io(error)
        }
    })
}

async fn highest_snapshot_seq(path: &Path) -> Result<u64, StoreError> {
    let mut highest = None;
    let mut entries = fs::read_dir(path).await.map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::BlockNotFound
        } else {
            StoreError::Io(error)
        }
    })?;
    while let Some(entry) = entries.next_entry().await? {
        if let Some(seq) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u64>().ok())
        {
            highest = Some(highest.map_or(seq, |current: u64| current.max(seq)));
        }
    }
    highest.ok_or(StoreError::BlockNotFound)
}

async fn read_operations(path: &Path) -> Result<Vec<OperationRecord>, StoreError> {
    let mut records = BTreeMap::new();
    let mut entries = fs::read_dir(path).await.map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::BlockNotFound
        } else {
            StoreError::Io(error)
        }
    })?;
    while let Some(entry) = entries.next_entry().await? {
        let Some(seq) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u64>().ok())
        else {
            continue;
        };
        let record: OperationRecord = serde_json::from_slice(&fs::read(entry.path()).await?)?;
        if record.seq != seq {
            return Err(StoreError::CorruptOperationLog);
        }
        if records.insert(seq, record).is_some() {
            return Err(StoreError::CorruptOperationLog);
        }
    }
    let records: Vec<_> = records.into_values().collect();
    for (index, record) in records.iter().enumerate() {
        if record.seq != index as u64 + 1 {
            return Err(StoreError::CorruptOperationLog);
        }
    }
    Ok(records)
}

async fn write_new_file(path: PathBuf, data: Vec<u8>) -> Result<(), StoreError> {
    use tokio::io::AsyncWriteExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(&data).await?;
    file.flush().await?;
    Ok(())
}

#[derive(Debug)]
enum StoreError {
    BlockAlreadyExists,
    BlockNotFound,
    ConflictingOperationId,
    InvalidMessage(String),
    InvalidSeq { expected: u64, actual: u64 },
    ParentCycle,
    ParentMissingReference,
    ReferencedBlockNotFound,
    CorruptOperationLog,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl StoreError {
    fn to_response(&self, request_id: Uuid, command: CommandKind, id: Uuid) -> ServerMessage {
        ServerMessage::Error {
            request_id: Some(request_id),
            command: Some(command),
            id: Some(id),
            code: self.code(),
            message: self.to_string(),
            expected_seq: match self {
                Self::InvalidSeq { expected, .. } => Some(*expected),
                _ => None,
            },
        }
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::BlockAlreadyExists => ErrorCode::BlockAlreadyExists,
            Self::BlockNotFound => ErrorCode::BlockNotFound,
            Self::ConflictingOperationId => ErrorCode::ConflictingOperationId,
            Self::InvalidMessage(_) => ErrorCode::InvalidMessage,
            Self::InvalidSeq { .. } => ErrorCode::InvalidSeq,
            Self::ParentCycle => ErrorCode::ParentCycle,
            Self::ParentMissingReference => ErrorCode::ParentMissingReference,
            Self::ReferencedBlockNotFound => ErrorCode::ReferencedBlockNotFound,
            Self::CorruptOperationLog | Self::Io(_) | Self::Json(_) => ErrorCode::StorageError,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockAlreadyExists => write!(formatter, "block already exists"),
            Self::BlockNotFound => write!(formatter, "block does not exist"),
            Self::ConflictingOperationId => {
                write!(formatter, "operation UUID reused with different data")
            }
            Self::InvalidMessage(message) => formatter.write_str(message),
            Self::InvalidSeq { expected, actual } => {
                write!(formatter, "invalid seq {actual}; expected {expected}")
            }
            Self::ParentCycle => write!(formatter, "parent assignment would create a cycle"),
            Self::ParentMissingReference => {
                write!(formatter, "parent does not reference the child block")
            }
            Self::ReferencedBlockNotFound => write!(formatter, "referenced block does not exist"),
            Self::CorruptOperationLog => write!(formatter, "operation log is not contiguous"),
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
pub enum ServerError {
    Io(std::io::Error),
    Json(serde_json::Error),
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
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
        Self::WebSocket(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

    #[tokio::test]
    async fn operation_ids_are_idempotent_and_conflicts_are_rejected() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();
        let id = Uuid::new_v4();
        store
            .create_block_unlocked(
                id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![1],
                "Block".into(),
                vec![],
            )
            .await
            .unwrap();
        let operation_id = Uuid::new_v4();

        assert!(matches!(
            store
                .update_block_unlocked(
                    id,
                    Some(1),
                    operation_id,
                    Uuid::new_v4(),
                    vec![2],
                    "Block".into(),
                    ReferenceDelta::default(),
                )
                .await
                .unwrap(),
            UpdateOutcome::Inserted(..)
        ));
        assert!(matches!(
            store
                .update_block_unlocked(
                    id,
                    Some(99),
                    operation_id,
                    Uuid::new_v4(),
                    vec![2],
                    "Block".into(),
                    ReferenceDelta::default(),
                )
                .await
                .unwrap(),
            UpdateOutcome::Duplicate(..)
        ));
        assert!(matches!(
            store
                .update_block_unlocked(
                    id,
                    Some(2),
                    operation_id,
                    Uuid::new_v4(),
                    vec![3],
                    "Block".into(),
                    ReferenceDelta::default(),
                )
                .await,
            Err(StoreError::ConflictingOperationId)
        ));
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn reads_replay_contiguous_operation_records() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();
        let id = Uuid::new_v4();
        let block_type = Uuid::new_v4();
        store
            .create_block_unlocked(
                id,
                block_type,
                Uuid::new_v4(),
                vec![1],
                "Block".into(),
                vec![],
            )
            .await
            .unwrap();
        store
            .update_block_unlocked(
                id,
                Some(1),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();
        store
            .update_block_unlocked(
                id,
                Some(2),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![3],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();

        let read = store.read_block_unlocked(id).await.unwrap();
        assert_eq!(read.block_type, block_type);
        assert_eq!(read.snapshot, vec![1]);
        assert_eq!(read.snapshot_seq, 0);
        assert_eq!(read.operations.len(), 2);
        assert_eq!(read.operations[0].seq, 1);
        assert_eq!(read.operations[1].seq, 2);
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn sequence_errors_include_the_expected_sequence() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();
        let id = Uuid::new_v4();
        store
            .create_block_unlocked(
                id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![],
                "Block".into(),
                vec![],
            )
            .await
            .unwrap();
        assert!(matches!(
            store
                .update_block_unlocked(
                    id,
                    Some(4),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    vec![],
                    "Block".into(),
                    ReferenceDelta::default(),
                )
                .await,
            Err(StoreError::InvalidSeq {
                expected: 1,
                actual: 4
            })
        ));
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn omitted_sequences_are_assigned_by_the_server() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();
        let id = Uuid::new_v4();
        store
            .create_block_unlocked(
                id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![],
                "Block".into(),
                vec![],
            )
            .await
            .unwrap();

        let first = store
            .update_block_unlocked(
                id,
                None,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![1],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();
        let second = store
            .update_block_unlocked(
                id,
                None,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();

        assert!(matches!(first, UpdateOutcome::Inserted(record, _) if record.seq == 1));
        assert!(matches!(second, UpdateOutcome::Inserted(record, _) if record.seq == 2));
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn explicit_sequences_cannot_be_applied_out_of_order() {
        let root = test_root();
        let store = BlockStore::new(root.clone());
        fs::create_dir_all(store.root()).await.unwrap();
        let id = Uuid::new_v4();
        store
            .create_block_unlocked(
                id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![],
                "Block".into(),
                vec![],
            )
            .await
            .unwrap();

        assert!(matches!(
            store
                .update_block_unlocked(
                    id,
                    Some(2),
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    vec![2],
                    "Block".into(),
                    ReferenceDelta::default(),
                )
                .await,
            Err(StoreError::InvalidSeq {
                expected: 1,
                actual: 2
            })
        ));
        store
            .update_block_unlocked(
                id,
                Some(1),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![1],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();
        store
            .update_block_unlocked(
                id,
                Some(2),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap();

        let read = store.read_block_unlocked(id).await.unwrap();
        assert_eq!(
            read.operations
                .iter()
                .map(|record| record.seq)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn shared_protocol_round_trips_over_websocket() {
        let root = test_root();
        let store = Arc::new(BlockStore::new(root.clone()));
        let watch_hub = Arc::new(WatchHub::new());
        fs::create_dir_all(store.root()).await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let store = Arc::clone(&store);
            let watch_hub = Arc::clone(&watch_hub);
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_connection(stream, store, watch_hub).await.unwrap();
            }
        });
        let mut client = test_connect(format!("ws://{addr}")).await;
        let id = Uuid::new_v4();
        let block_type = Uuid::new_v4();

        let create_request = Uuid::new_v4();
        send_message(
            &mut client,
            ClientMessage::CreateBlock {
                request_id: create_request,
                id,
                block_type,
                data: vec![1, 2],
                implicit_name: "Block".into(),
                references: vec![],
                watch: true,
            },
        )
        .await;
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::Ok {
                request_id,
                command: CommandKind::CreateBlock,
                ..
            } if request_id == create_request
        ));

        let operation_id = Uuid::new_v4();
        send_message(
            &mut client,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id,
                seq: Some(1),
                operation_id,
                operation: vec![3],
                implicit_name: "Block".into(),
                references: ReferenceDelta::default(),
            },
        )
        .await;
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::Ok {
                command: CommandKind::UpdateBlock,
                seq: Some(1),
                operation_id: Some(found),
                ..
            } if found == operation_id
        ));
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::BlockUpdated {
                operation: OperationRecord {
                    seq: 1,
                    operation_id: found,
                    ..
                },
                ..
            } if found == operation_id
        ));

        let read_request = Uuid::new_v4();
        send_message(
            &mut client,
            ClientMessage::ReadBlock {
                request_id: read_request,
                id,
                watch: true,
            },
        )
        .await;
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::ReadBlock {
                request_id,
                block_type: found_type,
                snapshot_seq: 0,
                operations,
                ..
            } if request_id == read_request
                && found_type == block_type
                && operations.len() == 1
                && operations[0].operation_id == operation_id
        ));

        client.close(None).await.unwrap();
        server.await.unwrap();
        fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn batch_is_acknowledged_before_watch_notifications() {
        let root = test_root();
        let store = Arc::new(BlockStore::new(root.clone()));
        let watch_hub = Arc::new(WatchHub::new());
        fs::create_dir_all(store.root()).await.unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let store = Arc::clone(&store);
            let watch_hub = Arc::clone(&watch_hub);
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_connection(stream, store, watch_hub).await.unwrap();
            }
        });
        let mut client = test_connect(format!("ws://{addr}")).await;
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        for id in [first, second] {
            send_message(
                &mut client,
                ClientMessage::CreateBlock {
                    request_id: Uuid::new_v4(),
                    id,
                    block_type: Uuid::new_v4(),
                    data: vec![],
                    implicit_name: "Block".into(),
                    references: vec![],
                    watch: true,
                },
            )
            .await;
            assert!(matches!(
                next_message(&mut client).await,
                ServerMessage::Ok {
                    command: CommandKind::CreateBlock,
                    ..
                }
            ));
        }

        let request_id = Uuid::new_v4();
        send_message(
            &mut client,
            ClientMessage::UpdateBatch {
                request_id,
                updates: vec![
                    block::BlockUpdate {
                        id: first,
                        seq: Some(1),
                        operation_id: Uuid::new_v4(),
                        operation: vec![1],
                        implicit_name: "First".into(),
                        references: ReferenceDelta::default(),
                    },
                    block::BlockUpdate {
                        id: second,
                        seq: Some(1),
                        operation_id: Uuid::new_v4(),
                        operation: vec![2],
                        implicit_name: "Second".into(),
                        references: ReferenceDelta::default(),
                    },
                ],
            },
        )
        .await;
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::BatchOk {
                request_id: found,
                operations,
                ..
            } if found == request_id && operations.len() == 2
        ));
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::BatchUpdated { operations }
                if operations.len() == 2
        ));

        client.close(None).await.unwrap();
        server.await.unwrap();
        fs::remove_dir_all(root).await.unwrap();
    }

    async fn send_message<S>(socket: &mut S, message: ClientMessage)
    where
        S: SinkExt<Message> + Unpin,
        S::Error: fmt::Debug,
    {
        socket
            .send(Message::Text(serde_json::to_string(&message).unwrap()))
            .await
            .unwrap();
    }

    async fn test_connect(
        url: String,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let mut request = url.into_client_request().unwrap();
        request.headers_mut().insert(
            ACCOUNT_HEADER,
            HeaderValue::from_str(&Uuid::new_v4().to_string()).unwrap(),
        );
        connect_async(request).await.unwrap().0
    }

    async fn next_message<S>(socket: &mut S) -> ServerMessage
    where
        S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        let message = socket.next().await.unwrap().unwrap();
        serde_json::from_str(&message.into_text().unwrap()).unwrap()
    }

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("block-server-test-{}", Uuid::new_v4()))
    }
}
