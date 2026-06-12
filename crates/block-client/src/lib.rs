use std::{
    collections::{HashMap, VecDeque},
    ops::Deref,
    process,
    sync::{mpsc, Arc, OnceLock},
    thread,
};

use block::{Block, ClientMessage, CommandKind, ErrorCode, OperationRecord, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::{oneshot, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

pub struct BlockClient {
    commands: mpsc::Sender<WorkerCommand>,
    connected: Arc<OnceLock<()>>,
}

impl BlockClient {
    pub fn new() -> Self {
        let (commands, command_rx) = mpsc::channel();
        let connected = Arc::new(OnceLock::new());
        thread::Builder::new()
            .name("block-client".into())
            .spawn(move || worker_main(command_rx))
            .unwrap_or_else(|error| fatal(format!("failed to spawn block client worker: {error}")));
        Self {
            commands,
            connected,
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
        let block = Arc::new(TypedBlock::<B>::created(id, Arc::clone(&shared), initial));
        self.send(WorkerCommand::AddBlock(block.clone()));
        BlockHandle {
            id,
            block,
            commands: self.commands.clone(),
        }
    }

    pub fn get_block<B: Block>(&self, id: Uuid) -> BlockHandle<B> {
        let shared = Arc::new(BlockShared {
            value: RwLock::new(None),
        });
        let block = Arc::new(TypedBlock::<B>::unresolved(id, Arc::clone(&shared)));
        self.send(WorkerCommand::AddBlock(block.clone()));
        BlockHandle {
            id,
            block,
            commands: self.commands.clone(),
        }
    }

    pub async fn synchronized(&self) {
        let (completed, completion) = oneshot::channel();
        self.send(WorkerCommand::Synchronize(completed));
        completion
            .await
            .unwrap_or_else(|_| fatal("block client worker stopped before synchronizing"));
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
    id: Uuid,
    block: Arc<TypedBlock<B>>,
    commands: mpsc::Sender<WorkerCommand>,
}

impl<B: Block> Clone for BlockHandle<B> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            block: Arc::clone(&self.block),
            commands: self.commands.clone(),
        }
    }
}

impl<B: Block> BlockHandle<B> {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn read(&self) -> Option<BlockReadGuard<'_, B>> {
        let guard = self.block.shared.value.read();
        if guard.is_none() {
            return None;
        }
        Some(BlockReadGuard {
            guard: RwLockReadGuard::map(guard, |value| value.as_ref().unwrap()),
        })
    }

    pub fn operate(&self, operation: B::Operation) {
        self.block.local_operation(operation);
        self.commands
            .send(WorkerCommand::Operate { id: self.id })
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
}

pub struct BlockReadGuard<'a, B: Block> {
    guard: MappedRwLockReadGuard<'a, B>,
}

impl<B: Block> Deref for BlockReadGuard<'_, B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

struct BlockShared<B: Block> {
    value: RwLock<Option<B>>,
}

enum WorkerCommand {
    Connect(String),
    AddBlock(Arc<dyn ErasedBlock>),
    Operate { id: Uuid },
    Synchronize(oneshot::Sender<()>),
}

fn worker_main(commands: mpsc::Receiver<WorkerCommand>) {
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

        let mut state = WorkerState::default();
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

#[derive(Default)]
struct WorkerState {
    connected: bool,
    blocks: HashMap<Uuid, Arc<dyn ErasedBlock>>,
    requests: HashMap<Uuid, PendingRequest>,
    outbound: VecDeque<ClientMessage>,
    synchronization_waiters: Vec<oneshot::Sender<()>>,
}

impl WorkerState {
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
            self.requests
                .insert(request_id, PendingRequest::Create { id });
            ClientMessage::CreateBlock {
                request_id,
                id,
                block_type: block.block_type_id(),
                data,
                watch: true,
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
                    _ => fatal("server response did not match its request"),
                }
            }
            ServerMessage::ReadBlock {
                request_id,
                id,
                block_type,
                snapshot,
                snapshot_seq,
                operations,
                ..
            } => {
                match self.requests.remove(&request_id) {
                    Some(PendingRequest::Read { id: expected }) if expected == id => {}
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
                let block = self
                    .blocks
                    .get(&id)
                    .unwrap_or_else(|| fatal(format!("update for unknown block {id}")));
                block.remote_operation(operation);
                self.maybe_send_update(id);
            }
            ServerMessage::Presence { .. } => {}
        }
    }

    fn finish_synchronization(&mut self) {
        if !self.connected
            || !self.requests.is_empty()
            || !self.outbound.is_empty()
            || self.blocks.values().any(|block| !block.is_synchronized())
        {
            return;
        }

        for completed in self.synchronization_waiters.drain(..) {
            let _ = completed.send(());
        }
    }
}

enum PendingRequest {
    Create { id: Uuid },
    Read { id: Uuid },
    Update { id: Uuid, operation_id: Uuid },
}

struct OutboundUpdate {
    seq: u64,
    operation_id: Uuid,
    operation: Vec<u8>,
}

trait ErasedBlock: Send + Sync {
    fn id(&self) -> Uuid;
    fn block_type_id(&self) -> Uuid;
    fn initial_data(&self) -> Option<Vec<u8>>;
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
}

struct TypedState<B: Block> {
    initial: Option<B>,
    confirmed: Option<B>,
    confirmed_seq: u64,
    pending: VecDeque<PendingOperation<B::Operation>>,
    in_flight: Option<Uuid>,
    ready: bool,
}

struct PendingOperation<O> {
    id: Uuid,
    operation: O,
}

impl<B: Block> TypedBlock<B> {
    fn created(id: Uuid, shared: Arc<BlockShared<B>>, initial: B) -> Self {
        Self {
            id,
            shared,
            state: RwLock::new(TypedState {
                initial: Some(initial.clone()),
                confirmed: Some(initial),
                confirmed_seq: 0,
                pending: VecDeque::new(),
                in_flight: None,
                ready: false,
            }),
            loaded: watch::channel(false).0,
        }
    }

    fn unresolved(id: Uuid, shared: Arc<BlockShared<B>>) -> Self {
        Self {
            id,
            shared,
            state: RwLock::new(TypedState {
                initial: None,
                confirmed: None,
                confirmed_seq: 0,
                pending: VecDeque::new(),
                in_flight: None,
                ready: false,
            }),
            loaded: watch::channel(false).0,
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
    }

    fn local_operation(&self, operation: B::Operation) {
        let mut state = self.state.write();
        state.pending.push_back(PendingOperation {
            id: Uuid::new_v4(),
            operation: operation.clone(),
        });
        if let Some(value) = self.shared.value.write().as_mut() {
            B::apply_operation(value, &operation);
        }
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

    fn created(&self) {
        self.state.write().ready = true;
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
        state.confirmed = Some(value);
        state.confirmed_seq = seq;
        state.ready = true;
        self.rebuild_visible(&state);
        self.loaded.send_replace(true);
    }

    fn next_update(&self) -> Option<OutboundUpdate> {
        let mut state = self.state.write();
        if !state.ready || state.in_flight.is_some() {
            return None;
        }
        let pending = state.pending.front()?;
        let update = OutboundUpdate {
            seq: state.confirmed_seq + 1,
            operation_id: pending.id,
            operation: serde_json::to_vec(&pending.operation)
                .unwrap_or_else(|error| fatal(format!("failed to serialize operation: {error}"))),
        };
        state.in_flight = Some(pending.id);
        Some(update)
    }

    fn acknowledge(&self, operation_id: Uuid, seq: u64) {
        let mut state = self.state.write();
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
        state.in_flight = None;
        self.rebuild_visible(&state);
    }

    fn sequence_conflict(&self, operation_id: Uuid, expected_seq: u64) -> bool {
        let mut state = self.state.write();
        if state.in_flight != Some(operation_id) {
            fatal("sequence conflict referenced the wrong operation");
        }
        state.in_flight = None;
        state.confirmed_seq + 1 >= expected_seq
    }

    fn remote_operation(&self, record: OperationRecord) {
        let mut state = self.state.write();
        if record.seq <= state.confirmed_seq {
            return;
        }
        if record.seq != state.confirmed_seq + 1 {
            fatal("received noncontiguous watched operation");
        }

        if state
            .pending
            .front()
            .is_some_and(|pending| pending.id == record.operation_id)
        {
            let pending = state.pending.pop_front().unwrap();
            B::apply_operation(state.confirmed.as_mut().unwrap(), &pending.operation);
            state.confirmed_seq = record.seq;
            state.in_flight = None;
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
        self.rebuild_visible(&state);
    }

    fn is_synchronized(&self) -> bool {
        let state = self.state.read();
        state.ready && state.pending.is_empty() && state.in_flight.is_none()
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
        );
        block.created();
        block.local_operation(CounterOperation::Add(2));
        block.local_operation(CounterOperation::Add(3));
        let first = block.next_update().unwrap();

        block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            operation: serde_json::to_vec(&CounterOperation::Add(10)).unwrap(),
        });

        assert_eq!(shared.value.read().as_ref().unwrap().count, 15);
        assert!(block.next_update().is_none());
        assert!(block.sequence_conflict(first.operation_id, 2));
        assert_eq!(block.next_update().unwrap().seq, 2);
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
        );
        block.created();
        block.local_operation(CounterOperation::Add(4));
        let update = block.next_update().unwrap();
        block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: update.operation_id,
            operation: update.operation,
        });
        block.acknowledge(update.operation_id, 1);

        assert_eq!(shared.value.read().as_ref().unwrap().count, 4);
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
                                }],
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
