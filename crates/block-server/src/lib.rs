use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use block::{
    Account, BlockOperation, BlockParent, BlockReference, BlockReferenceList, ClientMessage,
    CommandKind, ErrorCode, ManagementClientMessage, ManagementErrorCode, ManagementServerMessage,
    OperationRecord, ReferenceDelta, ServerMessage, MAX_NAME_BYTES,
};
use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
    task::JoinSet,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        Message,
    },
};
use uuid::Uuid;

const ACCOUNT_HEADER: &str = "x-block-account-id";
const DATABASE_FILE: &str = "server.sqlite3";

pub async fn serve(listener: TcpListener, data_dir: impl Into<PathBuf>) -> Result<(), ServerError> {
    let root = data_dir.into();
    fs::create_dir_all(&root).await?;
    let store = Arc::new(BlockStore::open(root).await?);
    let watch_hub = Arc::new(WatchHub::new());
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer_addr) = accepted?;
                let store = Arc::clone(&store);
                let watch_hub = Arc::clone(&watch_hub);
                connections.spawn(async move {
                    if let Err(error) = handle_connection(stream, store, watch_hub).await {
                        eprintln!("connection {peer_addr} closed with error: {error}");
                    }
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = result {
                    eprintln!("connection task failed: {error}");
                }
            }
        }
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
            account_id = request
                .headers()
                .get(ACCOUNT_HEADER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| Uuid::parse_str(value).ok());
            Ok(response)
        },
    )
    .await?;
    let Some(account_id) = account_id else {
        return handle_management_connection(socket, store).await;
    };
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

async fn handle_management_connection(
    mut socket: tokio_tungstenite::WebSocketStream<TcpStream>,
    store: Arc<BlockStore>,
) -> Result<(), ServerError> {
    while let Some(message) = socket.next().await {
        let response = match message? {
            Message::Text(text) => handle_management_message(&store, &text).await,
            Message::Close(_) => break,
            Message::Ping(payload) => {
                socket.send(Message::Pong(payload)).await?;
                continue;
            }
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                ManagementServerMessage::Error {
                    request_id: None,
                    code: ManagementErrorCode::UnsupportedMessage,
                    message: "only JSON text websocket messages are supported".into(),
                }
            }
        };
        socket
            .send(Message::Text(serde_json::to_string(&response)?))
            .await?;
    }
    Ok(())
}

async fn handle_management_message(store: &BlockStore, text: &str) -> ManagementServerMessage {
    let message = match serde_json::from_str::<ManagementClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            return ManagementServerMessage::Error {
                request_id: None,
                code: ManagementErrorCode::InvalidMessage,
                message: format!("invalid management command JSON: {error}"),
            };
        }
    };
    let request_id = message.request_id();
    let result = match message {
        ManagementClientMessage::Register {
            email,
            display_name,
            ..
        } => store.register_account(email, display_name).await,
        ManagementClientMessage::Login { email, .. } => store.login_account(email).await,
    };
    match result {
        Ok(account) => ManagementServerMessage::Account {
            request_id,
            account,
        },
        Err(error) => error.to_response(request_id),
    }
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
    database: Mutex<Connection>,
    locks: Mutex<HashMap<Uuid, Arc<Mutex<()>>>>,
    dependencies: Mutex<DependencyState>,
}

impl BlockStore {
    #[cfg(test)]
    fn new(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).unwrap();
        let mut connection = Connection::open(root.join(DATABASE_FILE)).unwrap();
        initialize_database(&mut connection).unwrap();
        Self::from_connection(connection).unwrap()
    }

    async fn open(root: PathBuf) -> Result<Self, ServerError> {
        let mut connection = Connection::open(root.join(DATABASE_FILE))?;
        initialize_database(&mut connection)?;
        Self::from_connection(connection).map_err(ServerError::from)
    }

    fn from_connection(connection: Connection) -> Result<Self, StoreError> {
        let dependencies = load_dependencies(&connection)?;
        Ok(Self {
            database: Mutex::new(connection),
            locks: Mutex::new(HashMap::new()),
            dependencies: Mutex::new(dependencies),
        })
    }

    async fn lock_for(&self, id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        Arc::clone(locks.entry(id).or_insert_with(|| Arc::new(Mutex::new(()))))
    }

    async fn register_account(
        &self,
        email: String,
        display_name: String,
    ) -> Result<Account, ManagementStoreError> {
        let email = normalize_email(&email)?;
        let display_name = normalize_display_name(&display_name)?;
        let account = Account {
            id: Uuid::new_v4(),
            email,
            display_name,
        };
        let database = self.database.lock().await;
        let result = database.execute(
            "INSERT INTO accounts (id, email, display_name) VALUES (?1, ?2, ?3)",
            params![
                account.id.to_string(),
                &account.email,
                &account.display_name
            ],
        );
        match result {
            Ok(_) => Ok(account),
            Err(error)
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
            {
                Err(ManagementStoreError::EmailAlreadyRegistered)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn login_account(&self, email: String) -> Result<Account, ManagementStoreError> {
        let email = normalize_email(&email)?;
        let database = self.database.lock().await;
        let account = database
            .query_row(
                "SELECT id, email, display_name FROM accounts WHERE email = ?1",
                [&email],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(ManagementStoreError::AccountNotFound)?;
        Ok(Account {
            id: Uuid::parse_str(&account.0)
                .map_err(|error| ManagementStoreError::InvalidStorage(error.to_string()))?,
            email: account.1,
            display_name: account.2,
        })
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
        let mut updated = dependencies.clone();
        updated.blocks.insert(
            id,
            DependencyBlock {
                block_type,
                author,
                name: implicit_name.clone(),
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
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT INTO blocks (
                id, block_type, author, snapshot, snapshot_seq, name, explicit_name,
                parent_kind, parent_id
            ) VALUES (?1, ?2, ?3, ?4, 0, ?5, NULL, 0, NULL)",
            params![
                id.to_string(),
                block_type.to_string(),
                author.to_string(),
                data,
                implicit_name,
            ],
        )?;
        persist_dependencies(&transaction, &updated)?;
        transaction.commit()?;
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
        let (exists, records) = {
            let database = self.database.lock().await;
            let exists = database
                .query_row(
                    "SELECT 1 FROM blocks WHERE id = ?1",
                    [id.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            let records = exists
                .then(|| read_operations(&database, id))
                .transpose()?
                .unwrap_or_default();
            (exists, records)
        };
        if !exists {
            return Err(StoreError::BlockNotFound);
        }

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
        let mut dependencies = self.dependencies.lock().await;
        let mut updated = dependencies.clone();
        updated.apply_references(id, &record.references)?;
        let block = updated
            .blocks
            .get_mut(&id)
            .ok_or(StoreError::BlockNotFound)?;
        if block.explicit_name.is_none() {
            block.name = implicit_name;
        }
        let name = block.name.clone();
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT INTO operations (
                block_id, seq, operation_id, author, operation, reference_delta
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                u64_to_i64(record.seq)?,
                record.operation_id.to_string(),
                record.author.to_string(),
                &record.operation,
                serde_json::to_string(&record.references)?,
            ],
        )?;
        persist_dependencies(&transaction, &updated)?;
        transaction.commit()?;
        *dependencies = updated;
        Ok(UpdateOutcome::Inserted(record, name))
    }

    async fn read_block_unlocked(&self, id: Uuid) -> Result<BlockRead, StoreError> {
        let database = self.database.lock().await;
        let block = database
            .query_row(
                "SELECT block_type, snapshot, snapshot_seq FROM blocks WHERE id = ?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StoreError::BlockNotFound)?;
        let block_type = parse_uuid(&block.0)?;
        let snapshot = block.1;
        let snapshot_seq = i64_to_u64(block.2)?;
        let operations = read_operations(&database, id)?
            .into_iter()
            .filter(|record| record.seq > snapshot_seq)
            .collect();
        drop(database);
        let dependencies = self.dependencies.lock().await;
        let dependency = dependencies
            .blocks
            .get(&id)
            .ok_or(StoreError::BlockNotFound)?;
        Ok(BlockRead {
            block_type,
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
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        persist_dependencies(&transaction, &updated)?;
        transaction.commit()?;
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
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        persist_dependencies(&transaction, &updated)?;
        transaction.commit()?;
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

#[derive(Clone, Default)]
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

#[derive(Clone)]
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

fn normalize_email(email: &str) -> Result<String, ManagementStoreError> {
    let email = email.trim().to_lowercase();
    let Some((local, domain)) = email.split_once('@') else {
        return Err(ManagementStoreError::InvalidEmail);
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return Err(ManagementStoreError::InvalidEmail);
    }
    Ok(email)
}

fn normalize_display_name(name: &str) -> Result<String, ManagementStoreError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(ManagementStoreError::InvalidName);
    }
    Ok(name.to_owned())
}

fn normalize_ids(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

fn initialize_database(connection: &mut Connection) -> Result<(), rusqlite::Error> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS blocks (
            id              TEXT PRIMARY KEY,
            block_type      TEXT NOT NULL,
            author          TEXT NOT NULL,
            snapshot        BLOB NOT NULL,
            snapshot_seq    INTEGER NOT NULL CHECK (snapshot_seq >= 0),
            name            TEXT NOT NULL,
            explicit_name   TEXT,
            parent_kind     INTEGER NOT NULL CHECK (parent_kind IN (0, 1, 2)),
            parent_id       TEXT,
            CHECK (
                (parent_kind = 2 AND parent_id IS NOT NULL)
                OR (parent_kind != 2 AND parent_id IS NULL)
            )
        );

        CREATE TABLE IF NOT EXISTS block_references (
            block_id        TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
            position        INTEGER NOT NULL CHECK (position >= 0),
            reference_id    TEXT NOT NULL REFERENCES blocks(id),
            PRIMARY KEY (block_id, position),
            UNIQUE (block_id, reference_id)
        );

        CREATE TABLE IF NOT EXISTS operations (
            block_id        TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
            seq             INTEGER NOT NULL CHECK (seq > 0),
            operation_id    TEXT NOT NULL,
            author          TEXT NOT NULL,
            operation       BLOB NOT NULL,
            reference_delta TEXT NOT NULL,
            PRIMARY KEY (block_id, seq),
            UNIQUE (block_id, operation_id)
        );

        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,
            email           TEXT NOT NULL UNIQUE,
            display_name    TEXT NOT NULL
        );
        ",
    )
}

enum ManagementStoreError {
    AccountNotFound,
    EmailAlreadyRegistered,
    InvalidEmail,
    InvalidName,
    InvalidStorage(String),
    Sqlite(rusqlite::Error),
}

impl ManagementStoreError {
    fn to_response(&self, request_id: Uuid) -> ManagementServerMessage {
        let (code, message) = match self {
            Self::AccountNotFound => (
                ManagementErrorCode::AccountNotFound,
                "account not found".into(),
            ),
            Self::EmailAlreadyRegistered => (
                ManagementErrorCode::EmailAlreadyRegistered,
                "email is already registered".into(),
            ),
            Self::InvalidEmail => (
                ManagementErrorCode::InvalidEmail,
                "invalid email address".into(),
            ),
            Self::InvalidName => (
                ManagementErrorCode::InvalidName,
                "invalid display name".into(),
            ),
            Self::InvalidStorage(message) => (ManagementErrorCode::StorageError, message.clone()),
            Self::Sqlite(error) => (ManagementErrorCode::StorageError, error.to_string()),
        };
        ManagementServerMessage::Error {
            request_id: Some(request_id),
            code,
            message,
        }
    }
}

impl From<rusqlite::Error> for ManagementStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

fn load_dependencies(connection: &Connection) -> Result<DependencyState, StoreError> {
    let mut state = DependencyState::default();
    {
        let mut statement = connection.prepare(
            "SELECT id, block_type, author, name, explicit_name, parent_kind, parent_id
             FROM blocks ORDER BY rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        for row in rows {
            let (id, block_type, author, name, explicit_name, parent_kind, parent_id) = row?;
            let parent = decode_parent(parent_kind, parent_id)?;
            state.blocks.insert(
                parse_uuid(&id)?,
                DependencyBlock {
                    block_type: parse_uuid(&block_type)?,
                    author: parse_uuid(&author)?,
                    name,
                    explicit_name,
                    parent,
                    references: Vec::new(),
                    backrefs: Vec::new(),
                },
            );
        }
    }

    let mut statement = connection.prepare(
        "SELECT block_id, reference_id
         FROM block_references ORDER BY block_id, position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (block_id, reference_id) = row?;
        let block_id = parse_uuid(&block_id)?;
        let reference_id = parse_uuid(&reference_id)?;
        state
            .blocks
            .get_mut(&block_id)
            .ok_or_else(|| StoreError::InvalidStorage("reference source is missing".into()))?
            .references
            .push(reference_id);
    }

    let references: Vec<_> = state
        .blocks
        .iter()
        .flat_map(|(&id, block)| {
            block
                .references
                .iter()
                .copied()
                .map(move |reference| (id, reference))
        })
        .collect();
    for (id, reference) in references {
        state
            .blocks
            .get_mut(&reference)
            .ok_or_else(|| StoreError::InvalidStorage("referenced block is missing".into()))?
            .backrefs
            .push(id);
    }
    Ok(state)
}

fn persist_dependencies(
    transaction: &Transaction<'_>,
    dependencies: &DependencyState,
) -> Result<(), StoreError> {
    transaction.execute("DELETE FROM block_references", [])?;
    for (&id, block) in &dependencies.blocks {
        let (parent_kind, parent_id) = encode_parent(block.parent);
        let updated = transaction.execute(
            "UPDATE blocks
             SET name = ?2, explicit_name = ?3, parent_kind = ?4, parent_id = ?5
             WHERE id = ?1",
            params![
                id.to_string(),
                block.name,
                block.explicit_name,
                parent_kind,
                parent_id,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidStorage(
                "dependency metadata references a missing block".into(),
            ));
        }
        for (position, reference) in block.references.iter().enumerate() {
            transaction.execute(
                "INSERT INTO block_references (block_id, position, reference_id)
                 VALUES (?1, ?2, ?3)",
                params![
                    id.to_string(),
                    i64::try_from(position).map_err(|_| {
                        StoreError::InvalidStorage("reference position exceeds SQLite range".into())
                    })?,
                    reference.to_string(),
                ],
            )?;
        }
    }
    Ok(())
}

fn read_operations(connection: &Connection, id: Uuid) -> Result<Vec<OperationRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT seq, operation_id, author, operation, reference_delta
         FROM operations WHERE block_id = ?1 ORDER BY seq",
    )?;
    let rows = statement.query_map([id.to_string()], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut records = Vec::new();
    for row in rows {
        let (seq, operation_id, author, operation, references) = row?;
        records.push(OperationRecord {
            seq: i64_to_u64(seq)?,
            operation_id: parse_uuid(&operation_id)?,
            author: parse_uuid(&author)?,
            operation,
            references: serde_json::from_str(&references)?,
        });
    }
    for (index, record) in records.iter().enumerate() {
        if record.seq != index as u64 + 1 {
            return Err(StoreError::CorruptOperationLog);
        }
    }
    Ok(records)
}

fn encode_parent(parent: BlockParent) -> (i64, Option<String>) {
    match parent {
        BlockParent::Orphaned => (0, None),
        BlockParent::Root => (1, None),
        BlockParent::Uuid(id) => (2, Some(id.to_string())),
    }
}

fn decode_parent(kind: i64, id: Option<String>) -> Result<BlockParent, StoreError> {
    match (kind, id) {
        (0, None) => Ok(BlockParent::Orphaned),
        (1, None) => Ok(BlockParent::Root),
        (2, Some(id)) => Ok(BlockParent::Uuid(parse_uuid(&id)?)),
        _ => Err(StoreError::InvalidStorage(
            "block has an invalid parent representation".into(),
        )),
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(value)
        .map_err(|error| StoreError::InvalidStorage(format!("invalid UUID in database: {error}")))
}

fn u64_to_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::InvalidStorage("sequence exceeds SQLite integer range".into()))
}

fn i64_to_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value)
        .map_err(|_| StoreError::InvalidStorage("negative sequence in database".into()))
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
    InvalidStorage(String),
    Sqlite(rusqlite::Error),
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
            Self::CorruptOperationLog
            | Self::InvalidStorage(_)
            | Self::Sqlite(_)
            | Self::Json(_) => ErrorCode::StorageError,
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
            Self::InvalidStorage(message) => write!(formatter, "invalid storage data: {message}"),
            Self::Sqlite(error) => write!(formatter, "SQLite storage error: {error}"),
            Self::Json(error) => write!(formatter, "storage JSON error: {error}"),
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
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
    Sqlite(rusqlite::Error),
    Storage(String),
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::Sqlite(error) => write!(formatter, "SQLite error: {error}"),
            Self::Storage(message) => write!(formatter, "storage error: {message}"),
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

impl From<rusqlite::Error> for ServerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<StoreError> for ServerError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Sqlite(error) => Self::Sqlite(error),
            StoreError::Json(error) => Self::Json(error),
            error => Self::Storage(error.to_string()),
        }
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ServerError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(error))
    }
}

#[cfg(test)]
mod tests;
