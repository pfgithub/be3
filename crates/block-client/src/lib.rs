use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    ops::Deref,
    process,
    sync::{mpsc, Arc, OnceLock},
    thread,
};

use block::{
    Block, BlockOperation, BlockUpdate, ClientMessage, CommandKind, ErrorCode, OperationRecord,
    ReferenceDelta, ServerMessage,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::{oneshot, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

pub mod text;

pub struct BlockClient {
    id: Uuid,
    commands: mpsc::Sender<WorkerCommand>,
    connected: Arc<OnceLock<()>>,
    access: Arc<RwLock<()>>,
}

impl BlockClient {
    pub fn new() -> Self {
        let (commands, command_rx) = mpsc::channel();
        let connected = Arc::new(OnceLock::new());
        let access = Arc::new(RwLock::new(()));
        let worker_access = Arc::clone(&access);
        thread::Builder::new()
            .name("block-client".into())
            .spawn(move || worker_main(command_rx, worker_access))
            .unwrap_or_else(|error| fatal(format!("failed to spawn block client worker: {error}")));
        Self {
            id: Uuid::new_v4(),
            commands,
            connected,
            access,
        }
    }

    pub fn connect(&self, url: impl Into<String>) {
        if self.connected.set(()).is_err() {
            fatal("BlockClient::connect may only be called once");
        }
        self.send(WorkerCommand::Connect(url.into()));
    }

    pub fn create_block<B: Block>(&self, initial: B) -> BlockHandle<B> {
        let id = Uuid::new_v4();
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(initial.clone())),
        });
        let block = Arc::new(TypedBlock::<B>::created(
            id,
            Arc::clone(&shared),
            initial,
            false,
        ));
        self.send(WorkerCommand::AddBlock(block.clone()));
        BlockHandle {
            client_id: self.id,
            id,
            block,
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }

    pub fn get_or_create_block<B: Block>(&self, id: Uuid, initial: B) -> BlockHandle<B> {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(initial.clone())),
        });
        let block = Arc::new(TypedBlock::<B>::created(
            id,
            Arc::clone(&shared),
            initial,
            true,
        ));
        self.send(WorkerCommand::AddBlock(block.clone()));
        BlockHandle {
            client_id: self.id,
            id,
            block,
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }

    pub fn get_block<B: Block>(&self, id: Uuid) -> BlockHandle<B> {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(None),
        });
        let block = Arc::new(TypedBlock::<B>::unresolved(id, Arc::clone(&shared)));
        self.send(WorkerCommand::AddBlock(block.clone()));
        BlockHandle {
            client_id: self.id,
            id,
            block,
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }

    pub fn batch(&self, update: impl FnOnce(&mut BlockBatch<'_>)) {
        let mut batch = BlockBatch {
            client_id: self.id,
            commands: self.commands.clone(),
            _access: self.access.write(),
            ids: Vec::new(),
        };
        update(&mut batch);
    }

    pub async fn synchronized(&self) {
        let (completed, completion) = oneshot::channel();
        self.send(WorkerCommand::Synchronize(completed));
        completion
            .await
            .unwrap_or_else(|_| fatal("block client worker stopped before synchronizing"));
    }

    pub async fn orphaned_blocks(&self) -> Vec<Uuid> {
        let (completed, completion) = oneshot::channel();
        self.send(WorkerCommand::ListOrphanedBlocks(completed));
        completion
            .await
            .unwrap_or_else(|_| fatal("block client worker stopped before listing orphaned blocks"))
    }

    fn send(&self, command: WorkerCommand) {
        self.commands
            .send(command)
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }
}

impl Default for BlockClient {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BlockHandle<B: Block> {
    client_id: Uuid,
    id: Uuid,
    block: Arc<TypedBlock<B>>,
    commands: mpsc::Sender<WorkerCommand>,
    access: Arc<RwLock<()>>,
}

impl<B: Block> Clone for BlockHandle<B> {
    fn clone(&self) -> Self {
        Self {
            client_id: self.client_id,
            id: self.id,
            block: Arc::clone(&self.block),
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }
}

impl<B: Block> BlockHandle<B> {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn read(&self) -> Option<BlockReadGuard<'_, B>> {
        let access = self.access.read();
        let guard = self.block.shared.value.read();
        if guard.is_none() {
            return None;
        }
        Some(BlockReadGuard {
            _access: access,
            guard: RwLockReadGuard::map(guard, |value| value.as_ref().unwrap()),
        })
    }

    pub fn operate(&self, operation: B::Operation) {
        self.block.local_operation(operation);
        self.commands
            .send(WorkerCommand::Operate { id: self.id })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn set_parent(&self, parent: Option<Uuid>) {
        self.commands
            .send(WorkerCommand::SetBlockParent {
                id: self.id,
                parent,
            })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub async fn loaded(&self) {
        let mut loaded = self.block.loaded.subscribe();
        while !*loaded.borrow_and_update() {
            loaded
                .changed()
                .await
                .unwrap_or_else(|_| fatal("block client worker stopped before loading the block"));
        }
    }

    pub async fn wait_until(&self, mut predicate: impl FnMut(&B) -> bool) {
        let mut changed = self.block.changed.subscribe();
        loop {
            if self.read().is_some_and(|block| predicate(&block)) {
                return;
            }
            changed
                .changed()
                .await
                .unwrap_or_else(|_| fatal("block client worker stopped while waiting for a block"));
        }
    }
}

pub struct BlockReadGuard<'a, B: Block> {
    _access: RwLockReadGuard<'a, ()>,
    guard: MappedRwLockReadGuard<'a, B>,
}

impl<B: Block> Deref for BlockReadGuard<'_, B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

pub struct BlockBatch<'a> {
    client_id: Uuid,
    commands: mpsc::Sender<WorkerCommand>,
    _access: RwLockWriteGuard<'a, ()>,
    ids: Vec<Uuid>,
}

impl BlockBatch<'_> {
    pub fn operate<B: Block>(&mut self, block: &BlockHandle<B>, operation: B::Operation) {
        if block.client_id != self.client_id {
            fatal("cannot batch a block owned by another client");
        }
        if self.ids.contains(&block.id) {
            fatal("a batch may contain at most one operation per block");
        }
        block.block.local_operation(operation);
        self.ids.push(block.id);
    }
}

impl Drop for BlockBatch<'_> {
    fn drop(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        self.commands
            .send(WorkerCommand::OperateBatch {
                ids: std::mem::take(&mut self.ids),
            })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }
}

struct BlockShared<B: Block> {
    value: RwLock<Option<B>>,
}

enum WorkerCommand {
    Connect(String),
    AddBlock(Arc<dyn ErasedBlock>),
    Operate { id: Uuid },
    OperateBatch { ids: Vec<Uuid> },
    SetBlockParent { id: Uuid, parent: Option<Uuid> },
    ListOrphanedBlocks(oneshot::Sender<Vec<Uuid>>),
    Synchronize(oneshot::Sender<()>),
}

fn worker_main(commands: mpsc::Receiver<WorkerCommand>, access: Arc<RwLock<()>>) {
    let runtime = tokio::runtime::Runtime::new()
        .unwrap_or_else(|error| fatal(format!("failed to create block client runtime: {error}")));
    runtime.block_on(async move {
        let (async_tx, mut async_rx) = tokio_mpsc::unbounded_channel();
        thread::Builder::new()
            .name("block-client-command-forwarder".into())
            .spawn(move || {
                while let Ok(command) = commands.recv() {
                    if async_tx.send(command).is_err() {
                        return;
                    }
                }
            })
            .unwrap_or_else(|error| fatal(format!("failed to spawn command forwarder: {error}")));

        let mut state = WorkerState::new(access);
        while let Some(command) = async_rx.recv().await {
            match command {
                WorkerCommand::Connect(url) => {
                    if run_connected(url, &mut state, &mut async_rx).await {
                        return;
                    }
                    fatal("block server connection closed");
                }
                command => state.handle_command(command),
            }
        }
    });
}

async fn run_connected(
    url: String,
    state: &mut WorkerState,
    commands: &mut tokio_mpsc::UnboundedReceiver<WorkerCommand>,
) -> bool {
    let (socket, _) = connect_async(&url)
        .await
        .unwrap_or_else(|error| fatal(format!("failed to connect to {url}: {error}")));
    let (mut sink, mut source) = socket.split();
    state.connected = true;
    state.queue_initial_requests();

    loop {
        while let Some(message) = state.outbound.pop_front() {
            let text = serde_json::to_string(&message).unwrap_or_else(|error| {
                fatal(format!("failed to serialize client message: {error}"))
            });
            sink.send(Message::Text(text))
                .await
                .unwrap_or_else(|error| fatal(format!("failed to send block message: {error}")));
        }

        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    return true;
                };
                if matches!(command, WorkerCommand::Connect(_)) {
                    fatal("BlockClient::connect may only be called once");
                }
                state.handle_command(command);
                state.finish_synchronization();
            }
            message = source.next() => {
                let message = match message {
                    Some(Ok(message)) => message,
                    Some(Err(error)) => fatal(format!("block server connection failed: {error}")),
                    None => fatal("block server connection closed"),
                };
                match message {
                    Message::Text(text) => {
                        let message: ServerMessage = serde_json::from_str(&text)
                            .unwrap_or_else(|error| fatal(format!("invalid server message: {error}: {text}")));
                        state.handle_server_message(message);
                        state.finish_synchronization();
                    }
                    Message::Ping(payload) => {
                        sink.send(Message::Pong(payload)).await
                            .unwrap_or_else(|error| fatal(format!("failed to send pong: {error}")));
                    }
                    Message::Close(_) => return false,
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                        fatal("server sent an unsupported websocket message");
                    }
                }
            }
        }
    }
}

struct WorkerState {
    connected: bool,
    access: Arc<RwLock<()>>,
    blocks: HashMap<Uuid, Arc<dyn ErasedBlock>>,
    requests: HashMap<Uuid, PendingRequest>,
    outbound: VecDeque<ClientMessage>,
    deferred: VecDeque<DeferredRequest>,
    synchronization_waiters: Vec<oneshot::Sender<()>>,
}

impl WorkerState {
    fn new(access: Arc<RwLock<()>>) -> Self {
        Self {
            connected: false,
            access,
            blocks: HashMap::new(),
            requests: HashMap::new(),
            outbound: VecDeque::new(),
            deferred: VecDeque::new(),
            synchronization_waiters: Vec::new(),
        }
    }

    fn handle_command(&mut self, command: WorkerCommand) {
        match command {
            WorkerCommand::Connect(_) => fatal("unexpected connect command"),
            WorkerCommand::AddBlock(block) => {
                let id = block.id();
                if self.blocks.insert(id, Arc::clone(&block)).is_some() {
                    fatal(format!("block {id} is already registered"));
                }
                if self.connected {
                    self.queue_initial_request(block);
                }
            }
            WorkerCommand::Operate { id } => {
                if !self.blocks.contains_key(&id) {
                    fatal(format!("unknown block {id}"));
                }
                self.maybe_send_update(id);
            }
            WorkerCommand::OperateBatch { ids } => {
                if ids.iter().any(|id| !self.blocks.contains_key(id)) {
                    fatal("batch contains an unknown block");
                }
                self.maybe_send_batch(ids);
            }
            WorkerCommand::SetBlockParent { id, parent } => {
                if !self.blocks.contains_key(&id) {
                    fatal(format!("unknown block {id}"));
                }
                self.deferred
                    .push_back(DeferredRequest::SetBlockParent { id, parent });
            }
            WorkerCommand::ListOrphanedBlocks(completed) => {
                self.deferred
                    .push_back(DeferredRequest::ListOrphanedBlocks(completed));
            }
            WorkerCommand::Synchronize(completed) => {
                self.synchronization_waiters.push(completed);
            }
        }
    }

    fn queue_initial_requests(&mut self) {
        let blocks: Vec<_> = self.blocks.values().cloned().collect();
        for block in blocks {
            self.queue_initial_request(block);
        }
    }

    fn queue_initial_request(&mut self, block: Arc<dyn ErasedBlock>) {
        let request_id = Uuid::new_v4();
        let id = block.id();
        let message = if let Some(data) = block.initial_data() {
            if block.get_or_create() {
                self.requests
                    .insert(request_id, PendingRequest::GetOrCreate { id });
                ClientMessage::GetOrCreateBlock {
                    request_id,
                    id,
                    block_type: block.block_type_id(),
                    data,
                    references: block.initial_references(),
                    watch: true,
                }
            } else {
                self.requests
                    .insert(request_id, PendingRequest::Create { id });
                ClientMessage::CreateBlock {
                    request_id,
                    id,
                    block_type: block.block_type_id(),
                    data,
                    references: block.initial_references(),
                    watch: true,
                }
            }
        } else {
            self.requests
                .insert(request_id, PendingRequest::Read { id });
            ClientMessage::ReadBlock {
                request_id,
                id,
                watch: true,
            }
        };
        self.outbound.push_back(message);
    }

    fn maybe_send_update(&mut self, id: Uuid) {
        if !self.connected {
            return;
        }
        loop {
            let block = self.blocks.get(&id).unwrap();
            let Some(update) = block.next_update() else {
                return;
            };
            let request_id = Uuid::new_v4();
            self.requests.insert(
                request_id,
                PendingRequest::Update {
                    id,
                    operation_id: update.operation_id,
                },
            );
            self.outbound.push_back(ClientMessage::UpdateBlock {
                request_id,
                id,
                seq: update.seq,
                operation_id: update.operation_id,
                operation: update.operation,
                references: update.references,
            });
        }
    }

    fn maybe_send_batch(&mut self, ids: Vec<Uuid>) {
        if !self.connected {
            return;
        }
        let mut updates = Vec::new();
        for id in ids {
            if let Some(update) = self.blocks[&id].next_update() {
                updates.push(BlockUpdate {
                    id,
                    seq: update.seq,
                    operation_id: update.operation_id,
                    operation: update.operation,
                    references: update.references,
                });
            }
        }
        if updates.is_empty() {
            return;
        }
        let request_id = Uuid::new_v4();
        self.requests.insert(
            request_id,
            PendingRequest::Batch {
                operations: updates
                    .iter()
                    .map(|update| (update.id, update.operation_id))
                    .collect(),
            },
        );
        self.outbound.push_back(ClientMessage::UpdateBatch {
            request_id,
            updates,
        });
    }

    fn handle_server_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Ok {
                request_id,
                command,
                id,
                seq,
                operation_id,
            } => {
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal(format!("response for unknown request {request_id}")));
                match (pending, command) {
                    (PendingRequest::Create { id: expected }, CommandKind::CreateBlock)
                        if expected == id =>
                    {
                        self.blocks[&id].created();
                        self.maybe_send_update(id);
                    }
                    (
                        PendingRequest::Update {
                            id: expected,
                            operation_id: expected_operation,
                        },
                        CommandKind::UpdateBlock,
                    ) if expected == id && Some(expected_operation) == operation_id => {
                        let seq =
                            seq.unwrap_or_else(|| fatal("update acknowledgement omitted seq"));
                        self.blocks[&id].acknowledge(expected_operation, seq);
                        self.maybe_send_update(id);
                    }
                    (
                        PendingRequest::SetBlockParent { id: expected },
                        CommandKind::SetBlockParent,
                    ) if expected == id => {}
                    _ => fatal("server response did not match its request"),
                }
            }
            ServerMessage::ReadBlock {
                request_id,
                command,
                id,
                block_type,
                snapshot,
                snapshot_seq,
                operations,
                ..
            } => {
                match (self.requests.remove(&request_id), command) {
                    (Some(PendingRequest::Read { id: expected }), CommandKind::ReadBlock)
                        if expected == id => {}
                    (
                        Some(PendingRequest::GetOrCreate { id: expected }),
                        CommandKind::GetOrCreateBlock,
                    ) if expected == id => {}
                    _ => fatal("read response did not match its request"),
                }
                let block = &self.blocks[&id];
                if block.block_type_id() != block_type {
                    fatal(format!(
                        "block {id} has type {block_type}, expected {}",
                        block.block_type_id()
                    ));
                }
                block.resolve(snapshot, snapshot_seq, operations);
                self.maybe_send_update(id);
            }
            ServerMessage::BatchOk {
                request_id,
                command: CommandKind::UpdateBatch,
                operations,
            } => {
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal(format!("response for unknown request {request_id}")));
                let PendingRequest::Batch {
                    operations: expected,
                } = pending
                else {
                    fatal("batch response did not match its request");
                };
                if expected.len() != operations.len() {
                    fatal("batch response operation count mismatch");
                }
                for ((expected_id, expected_operation), operation) in
                    expected.into_iter().zip(operations)
                {
                    if operation.id != expected_id
                        || operation.operation.operation_id != expected_operation
                    {
                        fatal("batch response operation mismatch");
                    }
                    self.blocks[&operation.id]
                        .acknowledge(operation.operation.operation_id, operation.operation.seq);
                    self.maybe_send_update(operation.id);
                }
            }
            ServerMessage::BatchOk { .. } => fatal("invalid batch response command"),
            ServerMessage::Error {
                request_id,
                command,
                id: response_id,
                code,
                message,
                expected_seq,
            } => {
                if code == ErrorCode::InvalidSeq && command == Some(CommandKind::UpdateBlock) {
                    let request_id =
                        request_id.unwrap_or_else(|| fatal("sequence error omitted request id"));
                    let pending = self
                        .requests
                        .remove(&request_id)
                        .unwrap_or_else(|| fatal("sequence error referenced unknown request"));
                    let PendingRequest::Update { id, operation_id } = pending else {
                        fatal("sequence error referenced a non-update request");
                    };
                    if Some(id) != response_id {
                        fatal("sequence error block mismatch");
                    }
                    let retry_now = self.blocks[&id].sequence_conflict(
                        operation_id,
                        expected_seq
                            .unwrap_or_else(|| fatal("sequence error omitted expected_seq")),
                    );
                    if retry_now {
                        self.maybe_send_update(id);
                    }
                    return;
                }
                fatal(format!(
                    "server rejected request {:?} for {:?}: {:?}: {}",
                    request_id, response_id, code, message
                ));
            }
            ServerMessage::BlockUpdated { id, operation } => {
                let access = Arc::clone(&self.access);
                let _access = access.write();
                let block = self
                    .blocks
                    .get(&id)
                    .unwrap_or_else(|| fatal(format!("update for unknown block {id}")));
                block.remote_operation(operation);
                drop(_access);
                self.maybe_send_update(id);
            }
            ServerMessage::BatchUpdated { operations } => {
                let access = Arc::clone(&self.access);
                let _access = access.write();
                let mut ids = Vec::with_capacity(operations.len());
                for BlockOperation { id, operation } in operations {
                    let block = self
                        .blocks
                        .get(&id)
                        .unwrap_or_else(|| fatal(format!("update for unknown block {id}")));
                    block.remote_operation(operation);
                    ids.push(id);
                }
                drop(_access);
                for id in ids {
                    self.maybe_send_update(id);
                }
            }
            ServerMessage::Presence { .. } => {}
            ServerMessage::OrphanedBlocks { request_id, blocks } => {
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal("orphan response referenced an unknown request"));
                let PendingRequest::ListOrphanedBlocks(completed) = pending else {
                    fatal("orphan response did not match its request");
                };
                let _ = completed.send(blocks);
            }
        }
    }

    fn finish_synchronization(&mut self) {
        self.maybe_send_deferred();
        if !self.connected
            || !self.requests.is_empty()
            || !self.outbound.is_empty()
            || !self.deferred.is_empty()
            || self.blocks.values().any(|block| !block.is_synchronized())
        {
            return;
        }

        for completed in self.synchronization_waiters.drain(..) {
            let _ = completed.send(());
        }
    }

    fn maybe_send_deferred(&mut self) {
        if !self.connected
            || !self.requests.is_empty()
            || !self.outbound.is_empty()
            || self.blocks.values().any(|block| !block.is_synchronized())
        {
            return;
        }
        let Some(request) = self.deferred.pop_front() else {
            return;
        };
        let request_id = Uuid::new_v4();
        match request {
            DeferredRequest::SetBlockParent { id, parent } => {
                self.requests
                    .insert(request_id, PendingRequest::SetBlockParent { id });
                self.outbound.push_back(ClientMessage::SetBlockParent {
                    request_id,
                    id,
                    parent,
                });
            }
            DeferredRequest::ListOrphanedBlocks(completed) => {
                self.requests
                    .insert(request_id, PendingRequest::ListOrphanedBlocks(completed));
                self.outbound
                    .push_back(ClientMessage::ListOrphanedBlocks { request_id });
            }
        }
    }
}

enum PendingRequest {
    Create { id: Uuid },
    GetOrCreate { id: Uuid },
    Read { id: Uuid },
    Update { id: Uuid, operation_id: Uuid },
    Batch { operations: Vec<(Uuid, Uuid)> },
    SetBlockParent { id: Uuid },
    ListOrphanedBlocks(oneshot::Sender<Vec<Uuid>>),
}

enum DeferredRequest {
    SetBlockParent { id: Uuid, parent: Option<Uuid> },
    ListOrphanedBlocks(oneshot::Sender<Vec<Uuid>>),
}

struct OutboundUpdate {
    seq: Option<u64>,
    operation_id: Uuid,
    operation: Vec<u8>,
    references: ReferenceDelta,
}

trait ErasedBlock: Send + Sync {
    fn id(&self) -> Uuid;
    fn block_type_id(&self) -> Uuid;
    fn initial_data(&self) -> Option<Vec<u8>>;
    fn initial_references(&self) -> Vec<Uuid>;
    fn get_or_create(&self) -> bool;
    fn created(&self);
    fn resolve(&self, snapshot: Vec<u8>, snapshot_seq: u64, operations: Vec<OperationRecord>);
    fn next_update(&self) -> Option<OutboundUpdate>;
    fn acknowledge(&self, operation_id: Uuid, seq: u64);
    fn sequence_conflict(&self, operation_id: Uuid, expected_seq: u64) -> bool;
    fn remote_operation(&self, operation: OperationRecord);
    fn is_synchronized(&self) -> bool;
}

struct TypedBlock<B: Block> {
    id: Uuid,
    shared: Arc<BlockShared<B>>,
    state: RwLock<TypedState<B>>,
    loaded: watch::Sender<bool>,
    changed: watch::Sender<()>,
}

struct TypedState<B: Block> {
    initial: Option<B>,
    get_or_create: bool,
    confirmed: Option<B>,
    confirmed_seq: u64,
    acknowledged_seq: u64,
    pending: VecDeque<PendingOperation<B::Operation>>,
    in_flight: HashSet<Uuid>,
    buffered: BTreeMap<u64, OperationRecord>,
    ready: bool,
}

struct PendingOperation<O> {
    id: Uuid,
    operation: O,
    references: ReferenceDelta,
}

impl<B: Block> TypedBlock<B> {
    fn created(id: Uuid, shared: Arc<BlockShared<B>>, initial: B, get_or_create: bool) -> Self {
        Self {
            id,
            shared,
            state: RwLock::new(TypedState {
                initial: Some(initial.clone()),
                get_or_create,
                confirmed: Some(initial),
                confirmed_seq: 0,
                acknowledged_seq: 0,
                pending: VecDeque::new(),
                in_flight: HashSet::new(),
                buffered: BTreeMap::new(),
                ready: false,
            }),
            loaded: watch::channel(false).0,
            changed: watch::channel(()).0,
        }
    }

    fn unresolved(id: Uuid, shared: Arc<BlockShared<B>>) -> Self {
        Self {
            id,
            shared,
            state: RwLock::new(TypedState {
                initial: None,
                get_or_create: false,
                confirmed: None,
                confirmed_seq: 0,
                acknowledged_seq: 0,
                pending: VecDeque::new(),
                in_flight: HashSet::new(),
                buffered: BTreeMap::new(),
                ready: false,
            }),
            loaded: watch::channel(false).0,
            changed: watch::channel(()).0,
        }
    }

    fn rebuild_visible(&self, state: &TypedState<B>) {
        let Some(mut value) = state.confirmed.clone() else {
            return;
        };
        for pending in &state.pending {
            B::apply_operation(&mut value, &pending.operation);
        }
        *self.shared.value.write() = Some(value);
        self.changed.send_replace(());
    }

    fn local_operation(&self, operation: B::Operation) {
        let mut state = self.state.write();
        let references = {
            let mut visible = self.shared.value.write();
            let value = visible
                .as_mut()
                .unwrap_or_else(|| fatal("cannot operate on an unresolved block"));
            let before = normalized_references(value.references());
            B::apply_operation(value, &operation);
            let after = normalized_references(value.references());
            self.changed.send_replace(());
            reference_delta(&before, &after)
        };
        state.pending.push_back(PendingOperation {
            id: Uuid::new_v4(),
            operation,
            references,
        });
    }
}

impl<B: Block> ErasedBlock for TypedBlock<B> {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type_id(&self) -> Uuid {
        B::TYPE_ID
    }

    fn initial_data(&self) -> Option<Vec<u8>> {
        self.state.read().initial.as_ref().map(|initial| {
            serde_json::to_vec(initial)
                .unwrap_or_else(|error| fatal(format!("failed to serialize block: {error}")))
        })
    }

    fn initial_references(&self) -> Vec<Uuid> {
        self.state
            .read()
            .initial
            .as_ref()
            .map_or_else(Vec::new, |initial| {
                normalized_references(initial.references())
            })
    }

    fn get_or_create(&self) -> bool {
        self.state.read().get_or_create
    }

    fn created(&self) {
        let mut state = self.state.write();
        state.ready = true;
        if B::CRDT {
            state.confirmed = None;
        }
        drop(state);
        self.loaded.send_replace(true);
    }

    fn resolve(&self, snapshot: Vec<u8>, snapshot_seq: u64, operations: Vec<OperationRecord>) {
        let mut value: B = serde_json::from_slice(&snapshot).unwrap_or_else(|error| {
            fatal(format!("failed to deserialize block snapshot: {error}"))
        });
        let mut seq = snapshot_seq;
        for record in operations {
            if record.seq != seq + 1 {
                fatal("server returned noncontiguous block history");
            }
            let operation: B::Operation = serde_json::from_slice(&record.operation)
                .unwrap_or_else(|error| fatal(format!("failed to deserialize operation: {error}")));
            B::apply_operation(&mut value, &operation);
            seq = record.seq;
        }
        let mut state = self.state.write();
        if B::CRDT {
            for pending in &state.pending {
                B::apply_operation(&mut value, &pending.operation);
            }
            *self.shared.value.write() = Some(value);
            self.changed.send_replace(());
            state.confirmed = None;
        } else {
            state.confirmed = Some(value);
            Self::recompute_pending_references(&mut state);
            self.rebuild_visible(&state);
        }
        state.confirmed_seq = seq;
        state.ready = true;
        self.loaded.send_replace(true);
    }

    fn next_update(&self) -> Option<OutboundUpdate> {
        let mut state = self.state.write();
        if !state.ready || (!B::CRDT && !state.in_flight.is_empty()) {
            return None;
        }
        let pending = state
            .pending
            .iter()
            .find(|pending| !state.in_flight.contains(&pending.id))?;
        let pending_id = pending.id;
        let update = OutboundUpdate {
            seq: (!B::CRDT).then_some(state.confirmed_seq + 1),
            operation_id: pending_id,
            operation: serde_json::to_vec(&pending.operation)
                .unwrap_or_else(|error| fatal(format!("failed to serialize operation: {error}"))),
            references: pending.references.clone(),
        };
        state.in_flight.insert(pending_id);
        Some(update)
    }

    fn acknowledge(&self, operation_id: Uuid, seq: u64) {
        let mut state = self.state.write();
        if B::CRDT {
            let Some(index) = state
                .pending
                .iter()
                .position(|pending| pending.id == operation_id)
            else {
                state.in_flight.remove(&operation_id);
                state.acknowledged_seq = state.acknowledged_seq.max(seq);
                return;
            };
            state.pending.remove(index);
            state.in_flight.remove(&operation_id);
            state.acknowledged_seq = state.acknowledged_seq.max(seq);
            return;
        }
        if seq <= state.confirmed_seq {
            return;
        }
        let Some(front) = state.pending.front() else {
            fatal("acknowledged operation is not pending");
        };
        if front.id != operation_id {
            fatal("acknowledged operation is not at the front of the pending queue");
        }
        if seq != state.confirmed_seq + 1 {
            fatal("update acknowledgement is not contiguous");
        }
        let operation = state.pending.pop_front().unwrap().operation;
        B::apply_operation(state.confirmed.as_mut().unwrap(), &operation);
        state.confirmed_seq = seq;
        state.in_flight.remove(&operation_id);
        self.rebuild_visible(&state);
    }

    fn sequence_conflict(&self, operation_id: Uuid, expected_seq: u64) -> bool {
        if B::CRDT {
            fatal("CRDT update received a sequence conflict");
        }
        let mut state = self.state.write();
        if !state.in_flight.remove(&operation_id) {
            fatal("sequence conflict referenced the wrong operation");
        }
        state.confirmed_seq + 1 >= expected_seq
    }

    fn remote_operation(&self, record: OperationRecord) {
        let mut state = self.state.write();
        if record.seq <= state.confirmed_seq {
            return;
        }
        state.buffered.entry(record.seq).or_insert(record);
        loop {
            let next_seq = state.confirmed_seq + 1;
            let Some(record) = state.buffered.remove(&next_seq) else {
                break;
            };
            self.apply_remote_operation(&mut state, record);
        }
    }

    fn is_synchronized(&self) -> bool {
        let state = self.state.read();
        state.ready
            && state.pending.is_empty()
            && state.in_flight.is_empty()
            && state.buffered.is_empty()
            && (!B::CRDT || state.confirmed_seq >= state.acknowledged_seq)
    }
}

impl<B: Block> TypedBlock<B> {
    fn recompute_pending_references(state: &mut TypedState<B>) {
        let Some(mut value) = state.confirmed.clone() else {
            return;
        };
        for pending in &mut state.pending {
            let before = normalized_references(value.references());
            B::apply_operation(&mut value, &pending.operation);
            let after = normalized_references(value.references());
            pending.references = reference_delta(&before, &after);
        }
    }

    fn apply_remote_operation(&self, state: &mut TypedState<B>, record: OperationRecord) {
        if B::CRDT {
            let remote: B::Operation =
                serde_json::from_slice(&record.operation).unwrap_or_else(|error| {
                    fatal(format!("failed to deserialize remote operation: {error}"))
                });
            if let Some(value) = self.shared.value.write().as_mut() {
                B::apply_operation(value, &remote);
                self.changed.send_replace(());
            }
            if let Some(index) = state
                .pending
                .iter()
                .position(|pending| pending.id == record.operation_id)
            {
                state.pending.remove(index);
            }
            state.in_flight.remove(&record.operation_id);
            state.confirmed_seq = record.seq;
            return;
        }

        if state
            .pending
            .front()
            .is_some_and(|pending| pending.id == record.operation_id)
        {
            let pending = state.pending.pop_front().unwrap();
            B::apply_operation(state.confirmed.as_mut().unwrap(), &pending.operation);
            state.confirmed_seq = record.seq;
            state.in_flight.remove(&record.operation_id);
            self.rebuild_visible(&state);
            return;
        }

        let remote: B::Operation =
            serde_json::from_slice(&record.operation).unwrap_or_else(|error| {
                fatal(format!("failed to deserialize remote operation: {error}"))
            });
        B::apply_operation(state.confirmed.as_mut().unwrap(), &remote);
        state.confirmed_seq = record.seq;
        for pending in &mut state.pending {
            B::transform_operation(&mut pending.operation, &remote);
        }
        Self::recompute_pending_references(state);
        self.rebuild_visible(&state);
    }
}

fn normalized_references(mut references: Vec<Uuid>) -> Vec<Uuid> {
    references.sort_unstable();
    references.dedup();
    references
}

fn reference_delta(before: &[Uuid], after: &[Uuid]) -> ReferenceDelta {
    ReferenceDelta {
        added: after
            .iter()
            .filter(|id| before.binary_search(id).is_err())
            .copied()
            .collect(),
        removed: before
            .iter()
            .filter(|id| after.binary_search(id).is_err())
            .copied()
            .collect(),
    }
}

fn fatal(message: impl AsRef<str>) -> ! {
    eprintln!("fatal block client error: {}", message.as_ref());
    process::abort()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct Counter {
        count: i64,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    enum CounterOperation {
        Add(i64),
    }

    impl Block for Counter {
        type Operation = CounterOperation;
        const TYPE_ID: Uuid = Uuid::from_u128(1);

        fn apply_operation(block: &mut Self, operation: &Self::Operation) {
            let CounterOperation::Add(amount) = operation;
            block.count += amount;
        }

        fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}
    }

    #[test]
    fn created_blocks_are_immediately_readable_and_operate_optimistically() {
        let client = BlockClient::new();
        let block = client.create_block(Counter { count: 1 });
        assert_eq!(block.read().unwrap().count, 1);
        block.operate(CounterOperation::Add(2));
        assert_eq!(block.read().unwrap().count, 3);
    }

    #[test]
    fn fetched_blocks_are_none_until_resolved() {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(None),
        });
        let block = TypedBlock::<Counter>::unresolved(Uuid::new_v4(), Arc::clone(&shared));
        assert!(shared.value.read().is_none());
        block.resolve(
            serde_json::to_vec(&Counter { count: 2 }).unwrap(),
            0,
            vec![OperationRecord {
                seq: 1,
                operation_id: Uuid::new_v4(),
                operation: serde_json::to_vec(&CounterOperation::Add(3)).unwrap(),
                references: ReferenceDelta::default(),
            }],
        );
        assert_eq!(shared.value.read().as_ref().unwrap().count, 5);
    }

    #[test]
    fn a_read_guard_blocks_background_updates() {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(Counter { count: 0 })),
        });
        let block = Arc::new(TypedBlock::<Counter>::created(
            Uuid::new_v4(),
            Arc::clone(&shared),
            Counter { count: 0 },
            false,
        ));
        block.created();
        let read = shared.value.read();
        let block_for_thread = Arc::clone(&block);
        let (finished_tx, finished_rx) = mpsc::channel();
        let update = thread::spawn(move || {
            block_for_thread.remote_operation(OperationRecord {
                seq: 1,
                operation_id: Uuid::new_v4(),
                operation: serde_json::to_vec(&CounterOperation::Add(1)).unwrap(),
                references: ReferenceDelta::default(),
            });
            finished_tx.send(()).unwrap();
        });

        assert!(finished_rx.try_recv().is_err());
        drop(read);
        finished_rx.recv().unwrap();
        update.join().unwrap();
        assert_eq!(shared.value.read().as_ref().unwrap().count, 1);
    }

    #[test]
    fn remote_operations_rebuild_all_pending_optimistic_operations() {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(Counter { count: 0 })),
        });
        let block = TypedBlock::<Counter>::created(
            Uuid::new_v4(),
            Arc::clone(&shared),
            Counter { count: 0 },
            false,
        );
        block.created();
        block.local_operation(CounterOperation::Add(2));
        block.local_operation(CounterOperation::Add(3));
        let first = block.next_update().unwrap();

        block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            operation: serde_json::to_vec(&CounterOperation::Add(10)).unwrap(),
            references: ReferenceDelta::default(),
        });

        assert_eq!(shared.value.read().as_ref().unwrap().count, 15);
        assert!(block.next_update().is_none());
        assert!(block.sequence_conflict(first.operation_id, 2));
        assert_eq!(block.next_update().unwrap().seq, Some(2));
    }

    #[test]
    fn matching_broadcast_before_acknowledgement_is_applied_once() {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(Counter { count: 0 })),
        });
        let block = TypedBlock::<Counter>::created(
            Uuid::new_v4(),
            Arc::clone(&shared),
            Counter { count: 0 },
            false,
        );
        block.created();
        block.local_operation(CounterOperation::Add(4));
        let update = block.next_update().unwrap();
        block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: update.operation_id,
            operation: update.operation,
            references: ReferenceDelta::default(),
        });
        block.acknowledge(update.operation_id, 1);

        assert_eq!(shared.value.read().as_ref().unwrap().count, 4);
    }

    #[test]
    fn watched_operations_are_buffered_until_their_sequence_is_contiguous() {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(Counter { count: 0 })),
        });
        let block = TypedBlock::<Counter>::created(
            Uuid::new_v4(),
            Arc::clone(&shared),
            Counter { count: 0 },
            false,
        );
        block.created();

        block.remote_operation(OperationRecord {
            seq: 2,
            operation_id: Uuid::new_v4(),
            operation: serde_json::to_vec(&CounterOperation::Add(2)).unwrap(),
            references: ReferenceDelta::default(),
        });
        assert_eq!(shared.value.read().as_ref().unwrap().count, 0);

        block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            operation: serde_json::to_vec(&CounterOperation::Add(1)).unwrap(),
            references: ReferenceDelta::default(),
        });
        assert_eq!(shared.value.read().as_ref().unwrap().count, 3);
    }

    #[tokio::test]
    async fn wait_until_observes_current_and_future_values() {
        let client = BlockClient::new();
        let block = client.create_block(Counter { count: 1 });
        block.wait_until(|counter| counter.count == 1).await;

        let block_for_update = block.clone();
        let update = tokio::spawn(async move {
            tokio::task::yield_now().await;
            block_for_update.block.remote_operation(OperationRecord {
                seq: 1,
                operation_id: Uuid::new_v4(),
                operation: serde_json::to_vec(&CounterOperation::Add(2)).unwrap(),
                references: ReferenceDelta::default(),
            });
        });

        block.wait_until(|counter| counter.count == 3).await;
        update.await.unwrap();
    }

    #[tokio::test]
    async fn get_block_resolves_from_a_websocket_read_response() {
        let id = Uuid::new_v4();
        let (address_tx, address_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                    address_tx.send(listener.local_addr().unwrap()).unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = accept_async(stream).await.unwrap();
                    let request = socket.next().await.unwrap().unwrap();
                    let request: ClientMessage =
                        serde_json::from_str(&request.into_text().unwrap()).unwrap();
                    let ClientMessage::ReadBlock {
                        request_id,
                        id: requested_id,
                        ..
                    } = request
                    else {
                        panic!("expected read request");
                    };
                    assert_eq!(requested_id, id);
                    socket
                        .send(Message::Text(
                            serde_json::to_string(&ServerMessage::ReadBlock {
                                request_id,
                                command: CommandKind::ReadBlock,
                                id,
                                block_type: Counter::TYPE_ID,
                                snapshot: serde_json::to_vec(&Counter { count: 2 }).unwrap(),
                                snapshot_seq: 0,
                                operations: vec![OperationRecord {
                                    seq: 1,
                                    operation_id: Uuid::new_v4(),
                                    operation: serde_json::to_vec(&CounterOperation::Add(3))
                                        .unwrap(),
                                    references: ReferenceDelta::default(),
                                }],
                                parent: None,
                                references: Vec::new(),
                                backrefs: Vec::new(),
                            })
                            .unwrap(),
                        ))
                        .await
                        .unwrap();
                    while socket.next().await.is_some() {}
                });
        });

        let address = address_rx.recv().unwrap();
        let client = BlockClient::new();
        let block = client.get_block::<Counter>(id);
        assert!(block.read().is_none());
        client.connect(format!("ws://{address}"));
        tokio::time::timeout(Duration::from_secs(2), block.loaded())
            .await
            .unwrap();
        assert_eq!(block.read().unwrap().count, 5);

        drop(block);
        drop(client);
        server.join().unwrap();
    }
}
