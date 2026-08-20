use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    future::Future,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use argon2::{
    password_hash::{
        rand_core::OsRng as PasswordHashOsRng, PasswordHash, PasswordHasher, PasswordVerifier,
        SaltString,
    },
    Argon2,
};
use block::{
    Account, BlockAccess, BlockAccessEntry, BlockOperation, BlockParent, BlockReference,
    BlockReferenceList, ClientEnvelope, ClientMessage, CommandKind, ErrorCode,
    ManagementClientMessage, ManagementErrorCode, ManagementServerMessage, OperationRecord,
    ReferenceDelta, ServerEnvelope, ServerMessage, Workspace, WorkspaceInvitation, WorkspaceRole,
    MAX_PROPERTY_VALUE_BYTES,
};
use futures_util::{SinkExt, StreamExt};
use indexmap::IndexMap;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
    task::JoinSet,
};
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
    WebSocketStream,
};
use uuid::Uuid;

mod http;

use self::http::{PrefixedStream, Status};

/// Block connections identify themselves in the websocket URL's query string.
/// Browsers cannot set request headers on a websocket handshake, so the query
/// string is the only place a web client can put this.
const TOKEN_PARAMETER: &str = "token";
const WORKSPACE_PARAMETER: &str = "workspace";
/// Management commands are POSTed here as JSON; the websocket carries block
/// traffic only.
const MANAGEMENT_PATH: &str = "/api/management";
const DATABASE_FILE: &str = "server.sqlite3";
/// The shortest password `register_account` accepts.
const MIN_PASSWORD_BYTES: usize = 8;
/// How many random bytes back a session token before it is hex-encoded.
const TOKEN_BYTES: usize = 32;
const MAX_WEBSOCKET_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Controls that only make sense to change for a standalone, publicly hosted
/// server. The embedded server block-app starts for itself uses the default.
#[derive(Clone, Copy)]
pub struct ServerConfig {
    /// Whether `ManagementClientMessage::Register` is answered at all. An
    /// operator running a public instance can turn this off and provision
    /// accounts by hand instead, so a stranger cannot self-register.
    pub allow_registration: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            allow_registration: true,
        }
    }
}

pub async fn serve(listener: TcpListener, data_dir: impl Into<PathBuf>) -> Result<(), ServerError> {
    serve_with_config(listener, data_dir, ServerConfig::default()).await
}

pub async fn serve_with_config(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    config: ServerConfig,
) -> Result<(), ServerError> {
    serve_until_shutdown(listener, data_dir, config, std::future::pending()).await
}

pub async fn serve_until_shutdown(
    listener: TcpListener,
    data_dir: impl Into<PathBuf>,
    config: ServerConfig,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ServerError> {
    let root = data_dir.into();
    fs::create_dir_all(&root).await?;
    let store = Arc::new(BlockStore::open_with_config(root, config).await?);
    let watch_hub = Arc::new(WatchHub::new());
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
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
    connections.shutdown().await;
    Ok(())
}

/// Creates an account directly against the database at `data_dir`, bypassing
/// the management protocol entirely (and whatever `ServerConfig` a running
/// instance was started with). This is how an operator provisions accounts
/// by hand on a server that has registration turned off.
pub async fn add_account(
    data_dir: impl Into<PathBuf>,
    email: String,
    display_name: String,
    password: String,
) -> Result<Account, ServerError> {
    let root = data_dir.into();
    fs::create_dir_all(&root).await?;
    let store = BlockStore::open_with_config(root, ServerConfig::default()).await?;
    let (account, _token) = store
        .register_account(email, display_name, password)
        .await
        .map_err(|error| {
            let ManagementServerMessage::Error { message, .. } = error.to_response(Uuid::nil())
            else {
                unreachable!("to_response always returns Error")
            };
            ServerError::Storage(message)
        })?;
    Ok(account)
}

async fn handle_connection(
    mut stream: TcpStream,
    store: Arc<BlockStore>,
    watch_hub: Arc<WatchHub>,
) -> Result<(), ServerError> {
    let request = http::read_head(&mut stream).await?;
    if !request.head.is_websocket_upgrade() {
        return handle_management_request(stream, request, store).await;
    }
    let identity = match connection_identity(&request.head, &store).await {
        Some(identity) => identity,
        None => {
            // The upgrade is refused outright rather than accepted and then
            // dropped, so the client sees why it was turned away.
            reject_handshake(&mut stream).await?;
            return Err(ServerError::InvalidHandshake);
        }
    };
    let socket = accept_async_with_config(
        PrefixedStream::new(request.buffered, stream),
        Some(WebSocketConfig {
            max_frame_size: Some(MAX_WEBSOCKET_FRAME_BYTES),
            ..WebSocketConfig::default()
        }),
    )
    .await?;
    handle_block_connection(socket, store, watch_hub, identity).await
}

/// Resolves the account a block connection's token proves and the workspace
/// it claims, confirming the account is a member of that workspace.
async fn connection_identity(head: &http::RequestHead, store: &BlockStore) -> Option<Identity> {
    let token = head.query_parameter(TOKEN_PARAMETER)?;
    let workspace_id = head
        .query_parameter(WORKSPACE_PARAMETER)
        .and_then(|value| Uuid::parse_str(value).ok())?;
    let account_id = store.resolve_token(token).await.ok()?;
    let role = store.workspace_role(account_id, workspace_id).await.ok()?;
    Some(Identity {
        account_id,
        workspace_id,
        role,
    })
}

async fn reject_handshake(stream: &mut TcpStream) -> Result<(), ServerError> {
    let response = ServerMessage::Error {
        request_id: None,
        command: None,
        id: None,
        code: ErrorCode::PermissionDenied,
        message: "the account is not a member of this workspace".into(),
        expected_seq: None,
    };
    http::write_json_response(stream, Status::Forbidden, &serde_json::to_vec(&response)?).await?;
    Ok(())
}

async fn handle_block_connection(
    socket: WebSocketStream<PrefixedStream<TcpStream>>,
    store: Arc<BlockStore>,
    watch_hub: Arc<WatchHub>,
    identity: Identity,
) -> Result<(), ServerError> {
    let workspace_id = identity.workspace_id;
    let (mut sink, mut source) = socket.split();
    let (sender, mut outbound_rx) = mpsc::unbounded_channel();
    let mut clients = Clients::new(&watch_hub, sender);

    loop {
        tokio::select! {
            Some(message) = outbound_rx.recv() => {
                sink.send(Message::Text(serde_json::to_string(&message)?)).await?;
            }
            Some(message) = source.next() => {
                match message? {
                    Message::Text(text) => {
                        let envelope = match serde_json::from_str::<ClientEnvelope>(&text) {
                            Ok(envelope) => envelope,
                            Err(error) => {
                                let response = ServerEnvelope::new(None, invalid_message(error));
                                sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                                continue;
                            }
                        };
                        let ClientEnvelope { client, message: command } = envelope;
                        if let ClientMessage::CloseClient { request_id } = command {
                            let response =
                                clients.close(&store, &watch_hub, client, request_id).await;
                            let response = ServerEnvelope::new(client, response);
                            sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                            continue;
                        }
                        let (client_id, outbound) = clients.get_or_open(&watch_hub, client);
                        let (response, notification) =
                            handle_command(
                                &store,
                                &watch_hub,
                                client_id,
                                identity,
                                outbound,
                                command,
                            ).await;
                        let response = ServerEnvelope::new(client, response);
                        sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                        if let Some(notification) = notification {
                            match notification {
                                ServerMessage::BatchUpdated { operations } => {
                                    watch_hub.broadcast_batch(&store, workspace_id, operations).await;
                                }
                                notification => {
                                    watch_hub.broadcast(&store, workspace_id, notification).await;
                                }
                            }
                        }
                        watch_hub.broadcast_reference_lists(&store).await;
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => sink.send(Message::Pong(payload)).await?,
                    Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                        let response = ServerEnvelope::new(None, ServerMessage::Error {
                            request_id: None,
                            command: None,
                            id: None,
                            code: ErrorCode::UnsupportedMessage,
                            message: "only JSON text websocket messages are supported".into(),
                            expected_seq: None,
                        });
                        sink.send(Message::Text(serde_json::to_string(&response)?)).await?;
                    }
                }
            }
            else => break,
        }
    }

    clients.remove_all(&store, &watch_hub).await;
    Ok(())
}

/// Every client a single connection is serving. The connection always has one
/// of its own, addressed by an absent envelope id; the rest are opened by name
/// the first time a frame arrives for them and live until they are closed or
/// the connection ends.
struct Clients {
    sender: mpsc::UnboundedSender<ServerEnvelope>,
    connection: ClientId,
    carried: HashMap<Uuid, ClientId>,
}

impl Clients {
    fn new(watch_hub: &WatchHub, sender: mpsc::UnboundedSender<ServerEnvelope>) -> Self {
        Self {
            connection: watch_hub.next_client_id(),
            sender,
            carried: HashMap::new(),
        }
    }

    fn get_or_open(
        &mut self,
        watch_hub: &WatchHub,
        client: Option<Uuid>,
    ) -> (ClientId, OutboundMessages) {
        let client_id = match client {
            None => self.connection,
            Some(client) => *self
                .carried
                .entry(client)
                .or_insert_with(|| watch_hub.next_client_id()),
        };
        let outbound = OutboundMessages {
            client,
            sender: self.sender.clone(),
        };
        (client_id, outbound)
    }

    async fn close(
        &mut self,
        store: &BlockStore,
        watch_hub: &WatchHub,
        client: Option<Uuid>,
        request_id: Uuid,
    ) -> ServerMessage {
        let Some(client) = client else {
            return ServerMessage::Error {
                request_id: Some(request_id),
                command: Some(CommandKind::CloseClient),
                id: None,
                code: ErrorCode::UnsupportedMessage,
                message: "a connection's own client is closed with the connection".into(),
                expected_seq: None,
            };
        };
        if let Some(client_id) = self.carried.remove(&client) {
            watch_hub.remove_client(store, client_id).await;
        }
        ServerMessage::Ok {
            request_id,
            command: CommandKind::CloseClient,
            id: client,
            seq: None,
            operation_id: None,
        }
    }

    async fn remove_all(&mut self, store: &BlockStore, watch_hub: &WatchHub) {
        for client_id in std::iter::once(self.connection).chain(self.carried.values().copied()) {
            watch_hub.remove_client(store, client_id).await;
        }
    }
}

fn invalid_message(error: serde_json::Error) -> ServerMessage {
    ServerMessage::Error {
        request_id: None,
        command: None,
        id: None,
        code: ErrorCode::InvalidMessage,
        message: format!("invalid command JSON: {error}"),
        expected_seq: None,
    }
}

/// Answers one management command over HTTP. The connection carries a single
/// request and is closed afterwards.
async fn handle_management_request(
    mut stream: TcpStream,
    request: http::BufferedRequest,
    store: Arc<BlockStore>,
) -> Result<(), ServerError> {
    let (status, response) = if request.head.path != MANAGEMENT_PATH {
        (
            Status::NotFound,
            management_error(
                ManagementErrorCode::UnsupportedMessage,
                format!("{} is not a management endpoint", request.head.path),
            ),
        )
    } else if request.head.method == "OPTIONS" {
        // A browser asks permission before sending a management command, and
        // will not send the command at all unless the preflight is answered.
        http::write_preflight_response(&mut stream).await?;
        return Ok(());
    } else if request.head.method != "POST" {
        (
            Status::MethodNotAllowed,
            management_error(
                ManagementErrorCode::UnsupportedMessage,
                format!(
                    "management commands are sent as POST {MANAGEMENT_PATH}, not {} {}",
                    request.head.method, request.head.path
                ),
            ),
        )
    } else {
        match http::read_body(&mut stream, &request).await {
            Ok(body) => (Status::Ok, handle_management_message(&store, &body).await),
            Err(error) => (
                Status::BadRequest,
                management_error(ManagementErrorCode::InvalidMessage, error.to_string()),
            ),
        }
    };
    http::write_json_response(&mut stream, status, &serde_json::to_vec(&response)?).await?;
    Ok(())
}

fn management_error(code: ManagementErrorCode, message: String) -> ManagementServerMessage {
    ManagementServerMessage::Error {
        request_id: None,
        code,
        message,
    }
}

async fn handle_management_message(store: &BlockStore, body: &[u8]) -> ManagementServerMessage {
    let message = match serde_json::from_slice::<ManagementClientMessage>(body) {
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
    match message {
        ManagementClientMessage::Register {
            email,
            display_name,
            password,
            ..
        } => {
            if !store.allow_registration {
                return ManagementStoreError::RegistrationDisabled.to_response(request_id);
            }
            match store.register_account(email, display_name, password).await {
                Ok((account, token)) => ManagementServerMessage::Account {
                    request_id,
                    account,
                    token,
                },
                Err(error) => error.to_response(request_id),
            }
        }
        ManagementClientMessage::Login {
            email, password, ..
        } => match store.login_account(email, password).await {
            Ok((account, token)) => ManagementServerMessage::Account {
                request_id,
                account,
                token,
            },
            Err(error) => error.to_response(request_id),
        },
        ManagementClientMessage::Logout { token, .. } => match store.logout(&token).await {
            Ok(()) => ManagementServerMessage::Ok { request_id },
            Err(error) => error.to_response(request_id),
        },
        ManagementClientMessage::ListWorkspaces { token, .. } => {
            match store.resolve_token(&token).await {
                Ok(account_id) => match store.list_workspaces(account_id).await {
                    Ok(workspaces) => ManagementServerMessage::Workspaces {
                        request_id,
                        workspaces,
                    },
                    Err(error) => error.to_response(request_id),
                },
                Err(error) => error.to_response(request_id),
            }
        }
        ManagementClientMessage::CreateWorkspace { token, name, .. } => {
            match store.resolve_token(&token).await {
                Ok(account_id) => match store.create_workspace(account_id, name).await {
                    Ok(workspace) => ManagementServerMessage::Workspace {
                        request_id,
                        workspace,
                    },
                    Err(error) => error.to_response(request_id),
                },
                Err(error) => error.to_response(request_id),
            }
        }
        ManagementClientMessage::ListInvitations { token, .. } => {
            match store.resolve_token(&token).await {
                Ok(account_id) => match store.list_invitations(account_id).await {
                    Ok(invitations) => ManagementServerMessage::Invitations {
                        request_id,
                        invitations,
                    },
                    Err(error) => error.to_response(request_id),
                },
                Err(error) => error.to_response(request_id),
            }
        }
        ManagementClientMessage::Invite {
            token,
            workspace_id,
            email,
            role,
            ..
        } => match store.resolve_token(&token).await {
            Ok(account_id) => match store.invite(account_id, workspace_id, email, role).await {
                Ok(invitation) => ManagementServerMessage::Invitation {
                    request_id,
                    invitation,
                },
                Err(error) => error.to_response(request_id),
            },
            Err(error) => error.to_response(request_id),
        },
        ManagementClientMessage::RespondInvitation {
            token,
            invitation_id,
            accept,
            ..
        } => match store.resolve_token(&token).await {
            Err(error) => error.to_response(request_id),
            Ok(account_id) => match store
                .respond_invitation(account_id, invitation_id, accept)
                .await
            {
                Ok(()) => ManagementServerMessage::Ok { request_id },
                Err(error) => error.to_response(request_id),
            },
        },
    }
}

/// Everything a block connection is allowed to act as: who is connected, the
/// workspace they opened, and the role that decides whether per-block
/// permissions apply to them at all.
#[derive(Clone, Copy)]
struct Identity {
    account_id: Uuid,
    workspace_id: Uuid,
    role: WorkspaceRole,
}

fn permission_denied(request_id: Uuid, command: CommandKind, id: Uuid) -> ServerMessage {
    ServerMessage::Error {
        request_id: Some(request_id),
        command: Some(command),
        id: Some(id),
        code: ErrorCode::PermissionDenied,
        message: "you do not have permission to access this block".into(),
        expected_seq: None,
    }
}

fn not_watching(request_id: Uuid, command: CommandKind, id: Uuid) -> ServerMessage {
    ServerMessage::Error {
        request_id: Some(request_id),
        command: Some(command),
        id: Some(id),
        code: ErrorCode::NotWatching,
        message: "the block must be watched before posting presence for it".into(),
        expected_seq: None,
    }
}

async fn handle_command(
    store: &BlockStore,
    watch_hub: &WatchHub,
    client_id: ClientId,
    identity: Identity,
    outbound: OutboundMessages,
    command: ClientMessage,
) -> (ServerMessage, Option<ServerMessage>) {
    let Identity {
        account_id,
        workspace_id,
        ..
    } = identity;
    match command {
        ClientMessage::CreateBlock {
            request_id,
            id,
            block_type,
            data,
            properties,
            dynamic_artifact,
            references,
            watch,
        } => {
            // A new block belongs to its author, but it may only point at blocks
            // the author is at least allowed to know about.
            let access = store.access(identity).await;
            if references
                .iter()
                .any(|reference| *reference != id && !access.get(*reference).can_know_exists())
            {
                return (
                    permission_denied(request_id, CommandKind::CreateBlock, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            let response = match store
                .create_block_unlocked(
                    workspace_id,
                    id,
                    block_type,
                    account_id,
                    data,
                    properties,
                    dynamic_artifact,
                    references,
                )
                .await
            {
                Ok(()) => {
                    if watch {
                        watch_hub.watch(identity, id, client_id, outbound).await;
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
            properties,
            dynamic_artifact,
            references,
        } => {
            let access = store.access(identity).await;
            if !access.get(id).can_edit()
                || references
                    .after
                    .iter()
                    .any(|reference| !access.get(*reference).can_know_exists())
            {
                return (
                    permission_denied(request_id, CommandKind::UpdateBlock, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            match store
                .update_block_unlocked(
                    workspace_id,
                    id,
                    seq,
                    operation_id,
                    account_id,
                    operation,
                    properties,
                    dynamic_artifact,
                    references,
                )
                .await
            {
                Ok(UpdateOutcome::Inserted(record, properties)) => (
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::UpdateBlock,
                        id,
                        seq: Some(record.seq),
                        operation_id: Some(record.operation_id),
                    },
                    Some(ServerMessage::BlockUpdated {
                        id,
                        properties,
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
            let access = store.access(identity).await;
            let denied = updates.iter().find(|update| {
                !access.get(update.id).can_edit()
                    || update
                        .references
                        .after
                        .iter()
                        .any(|reference| !access.get(*reference).can_know_exists())
            });
            if let Some(denied) = denied {
                return (
                    permission_denied(request_id, CommandKind::UpdateBatch, denied.id),
                    None,
                );
            }
            let mut locks = Vec::with_capacity(ids.len());
            for id in ids {
                locks.push(store.lock_for(workspace_id, id).await);
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
                        workspace_id,
                        update.id,
                        update.seq,
                        update.operation_id,
                        account_id,
                        update.operation,
                        update.properties,
                        update.dynamic_artifact,
                        update.references,
                    )
                    .await
                {
                    Ok(UpdateOutcome::Inserted(operation, properties)) => {
                        let operation = BlockOperation {
                            id: update.id,
                            properties,
                            operation,
                        };
                        inserted.push(operation.clone());
                        operations.push(operation);
                    }
                    Ok(UpdateOutcome::Duplicate(operation, properties)) => {
                        operations.push(BlockOperation {
                            id: update.id,
                            properties,
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
            let access = store.access(identity).await.get(id);
            if !access.can_view() {
                return (
                    permission_denied(request_id, CommandKind::ReadBlock, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            let response = match store.read_block_unlocked(workspace_id, id).await {
                Ok(read) => {
                    if watch {
                        watch_hub.watch(identity, id, client_id, outbound).await;
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
                        properties: read.properties,
                        access,
                    }
                }
                Err(error) => error.to_response(request_id, CommandKind::ReadBlock, id),
            };
            (response, None)
        }
        ClientMessage::UnwatchBlock { request_id, id } => {
            watch_hub.unwatch(store, workspace_id, id, client_id).await;
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
        ClientMessage::SetPresence {
            request_id,
            id,
            presence_id,
            data,
        } => {
            if !store.access(identity).await.get(id).can_view() {
                return (
                    permission_denied(request_id, CommandKind::SetPresence, id),
                    None,
                );
            }
            if data
                .as_ref()
                .is_some_and(|data| data.len() > MAX_PROPERTY_VALUE_BYTES)
            {
                return (
                    ServerMessage::Error {
                        request_id: Some(request_id),
                        command: Some(CommandKind::SetPresence),
                        id: Some(id),
                        code: ErrorCode::InvalidMessage,
                        message: "presence value exceeds the maximum size".into(),
                        expected_seq: None,
                    },
                    None,
                );
            }
            let watching = match data {
                Some(data) => {
                    watch_hub
                        .post_presence(store, workspace_id, id, client_id, presence_id, data)
                        .await
                }
                None => {
                    watch_hub
                        .clear_presence(store, workspace_id, id, client_id, presence_id)
                        .await
                }
            };
            if !watching {
                return (not_watching(request_id, CommandKind::SetPresence, id), None);
            }
            (
                ServerMessage::Ok {
                    request_id,
                    command: CommandKind::SetPresence,
                    id,
                    seq: None,
                    operation_id: None,
                },
                None,
            )
        }
        ClientMessage::SetBlockParent {
            request_id,
            id,
            parent,
        } => {
            let access = store.access(identity).await;
            let parent_allowed = match parent {
                BlockParent::Orphaned | BlockParent::Root => true,
                BlockParent::Uuid(parent) => access.get(parent).can_edit(),
            };
            if !access.get(id).can_edit() || !parent_allowed {
                return (
                    permission_denied(request_id, CommandKind::SetBlockParent, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            let response = match store.set_parent_unlocked(workspace_id, id, parent).await {
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
        ClientMessage::SetBlockProperty {
            request_id,
            id,
            key,
            value,
        } => {
            if !store.access(identity).await.get(id).can_edit() {
                return (
                    permission_denied(request_id, CommandKind::SetBlockProperty, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            match store
                .set_property_unlocked(workspace_id, id, key, value)
                .await
            {
                Ok(value) => (
                    ServerMessage::Ok {
                        request_id,
                        command: CommandKind::SetBlockProperty,
                        id,
                        seq: None,
                        operation_id: None,
                    },
                    Some(ServerMessage::BlockPropertyUpdated { id, key, value }),
                ),
                Err(error) => (
                    error.to_response(request_id, CommandKind::SetBlockProperty, id),
                    None,
                ),
            }
        }
        ClientMessage::ListReferences {
            request_id,
            list,
            watch,
        } => {
            let blocks = store.references(identity, list).await;
            if watch {
                watch_hub
                    .watch_references(identity, list, client_id, outbound, blocks.clone())
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
            watch_hub
                .unwatch_references(identity, list, client_id)
                .await;
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
        ClientMessage::ListBlockAccess { request_id, id } => {
            if !store.access(identity).await.get(id).can_edit() {
                return (
                    permission_denied(request_id, CommandKind::ListBlockAccess, id),
                    None,
                );
            }
            let response = match store.block_access_entries(workspace_id, id).await {
                Ok(entries) => ServerMessage::BlockAccessList {
                    request_id,
                    command: CommandKind::ListBlockAccess,
                    id,
                    entries,
                },
                Err(error) => error.to_response(request_id, CommandKind::ListBlockAccess, id),
            };
            (response, None)
        }
        ClientMessage::SetBlockAccess {
            request_id,
            id,
            account_id: target_id,
            access,
        } => {
            if !store.access(identity).await.get(id).can_edit() {
                return (
                    permission_denied(request_id, CommandKind::SetBlockAccess, id),
                    None,
                );
            }
            let lock = store.lock_for(workspace_id, id).await;
            let _guard = lock.lock().await;
            let response = match store
                .set_block_access_unlocked(workspace_id, id, target_id, access)
                .await
            {
                Ok(()) => ServerMessage::Ok {
                    request_id,
                    command: CommandKind::SetBlockAccess,
                    id,
                    seq: None,
                    operation_id: None,
                },
                Err(error) => error.to_response(request_id, CommandKind::SetBlockAccess, id),
            };
            (response, None)
        }
        ClientMessage::CloseClient { .. } => unreachable!("closing is handled by the connection"),
    }
}

use block::ClientId;

/// Where a watcher's notifications go: the connection's outbound queue, plus
/// the client of that connection they are addressed to.
#[derive(Clone)]
struct OutboundMessages {
    client: Option<Uuid>,
    sender: mpsc::UnboundedSender<ServerEnvelope>,
}

impl OutboundMessages {
    fn send(&self, message: ServerMessage) {
        let _ = self.sender.send(ServerEnvelope::new(self.client, message));
    }
}

/// Identifies a watcher's view of the workspace. Two accounts watching the same
/// list can see different blocks, so their subscriptions are tracked apart.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct WatchIdentity {
    account_id: Uuid,
    workspace_id: Uuid,
    role: WorkspaceRole,
}

impl From<Identity> for WatchIdentity {
    fn from(identity: Identity) -> Self {
        Self {
            account_id: identity.account_id,
            workspace_id: identity.workspace_id,
            role: identity.role,
        }
    }
}

impl From<WatchIdentity> for Identity {
    fn from(identity: WatchIdentity) -> Self {
        Self {
            account_id: identity.account_id,
            workspace_id: identity.workspace_id,
            role: identity.role,
        }
    }
}

struct WatchHub {
    next_client_id: AtomicU64,
    watchers: Mutex<HashMap<(Uuid, Uuid), HashMap<ClientId, BlockWatch>>>,
    reference_watchers:
        Mutex<HashMap<(WatchIdentity, BlockReferenceList), HashMap<ClientId, ReferenceWatch>>>,
}

struct BlockWatch {
    identity: WatchIdentity,
    outbound: OutboundMessages,
    /// This client's own presence values for the block, keyed by presence id.
    /// Cleared automatically (and reported to every other watcher) when the
    /// client unwatches the block or disconnects.
    presence: HashMap<Uuid, Vec<u8>>,
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

    /// Starts `client_id` watching `id`, replaying every presence value
    /// already recorded for it (from other clients) on `outbound` before the
    /// watch is registered, so the new watcher is caught up immediately.
    async fn watch(
        &self,
        identity: Identity,
        id: Uuid,
        client_id: ClientId,
        outbound: OutboundMessages,
    ) {
        let mut watchers = self.watchers.lock().await;
        let entries = watchers.entry((identity.workspace_id, id)).or_default();
        for (&other_id, watch) in entries.iter() {
            for (&presence_id, data) in &watch.presence {
                outbound.send(ServerMessage::Presence {
                    id,
                    client_id: other_id,
                    presence_id,
                    data: Some(data.clone()),
                });
            }
        }
        entries.insert(
            client_id,
            BlockWatch {
                identity: identity.into(),
                outbound,
                presence: HashMap::new(),
            },
        );
    }

    /// Stops `client_id` watching `id` and clears whatever presence it had
    /// recorded there, notifying every other watcher of the block.
    async fn unwatch(&self, store: &BlockStore, workspace_id: Uuid, id: Uuid, client_id: ClientId) {
        let (presence, deliveries) = {
            let mut watchers = self.watchers.lock().await;
            let key = (workspace_id, id);
            let Some(entries) = watchers.get_mut(&key) else {
                return;
            };
            let Some(watch) = entries.remove(&client_id) else {
                return;
            };
            let deliveries = entries
                .values()
                .map(|watch| (watch.identity, watch.outbound.clone()))
                .collect::<Vec<_>>();
            if entries.is_empty() {
                watchers.remove(&key);
            }
            (watch.presence, deliveries)
        };
        Self::notify_presence_cleared(store, id, client_id, presence, &deliveries).await;
    }

    async fn remove_client(&self, store: &BlockStore, client_id: ClientId) {
        let mut cleared = Vec::new();
        {
            let mut watchers = self.watchers.lock().await;
            watchers.retain(|&(_, id), entries| {
                if let Some(watch) = entries.remove(&client_id) {
                    if !watch.presence.is_empty() {
                        let deliveries = entries
                            .values()
                            .map(|watch| (watch.identity, watch.outbound.clone()))
                            .collect::<Vec<_>>();
                        cleared.push((id, watch.presence, deliveries));
                    }
                }
                !entries.is_empty()
            });
        }
        {
            let mut watchers = self.reference_watchers.lock().await;
            watchers.retain(|_, entries| {
                entries.remove(&client_id);
                !entries.is_empty()
            });
        }
        for (id, presence, deliveries) in cleared {
            Self::notify_presence_cleared(store, id, client_id, presence, &deliveries).await;
        }
    }

    /// Records a presence value from a client that is currently watching
    /// `id`, broadcasting it to every other watcher. Returns `false` without
    /// effect if the client is not watching the block.
    async fn post_presence(
        &self,
        store: &BlockStore,
        workspace_id: Uuid,
        id: Uuid,
        client_id: ClientId,
        presence_id: Uuid,
        data: Vec<u8>,
    ) -> bool {
        let deliveries = {
            let mut watchers = self.watchers.lock().await;
            let Some(entries) = watchers.get_mut(&(workspace_id, id)) else {
                return false;
            };
            let Some(watch) = entries.get_mut(&client_id) else {
                return false;
            };
            watch.presence.insert(presence_id, data.clone());
            entries
                .iter()
                .filter(|(&other_id, _)| other_id != client_id)
                .map(|(_, watch)| (watch.identity, watch.outbound.clone()))
                .collect::<Vec<_>>()
        };
        let message = ServerMessage::Presence {
            id,
            client_id,
            presence_id,
            data: Some(data),
        };
        Self::deliver(store, id, &message, &deliveries).await;
        true
    }

    /// Clears a presence value from a client that is currently watching
    /// `id`, notifying every other watcher. Returns `false` without effect
    /// if the client is not watching the block.
    async fn clear_presence(
        &self,
        store: &BlockStore,
        workspace_id: Uuid,
        id: Uuid,
        client_id: ClientId,
        presence_id: Uuid,
    ) -> bool {
        let deliveries = {
            let mut watchers = self.watchers.lock().await;
            let Some(entries) = watchers.get_mut(&(workspace_id, id)) else {
                return false;
            };
            let Some(watch) = entries.get_mut(&client_id) else {
                return false;
            };
            watch.presence.remove(&presence_id);
            entries
                .iter()
                .filter(|(&other_id, _)| other_id != client_id)
                .map(|(_, watch)| (watch.identity, watch.outbound.clone()))
                .collect::<Vec<_>>()
        };
        let message = ServerMessage::Presence {
            id,
            client_id,
            presence_id,
            data: None,
        };
        Self::deliver(store, id, &message, &deliveries).await;
        true
    }

    async fn notify_presence_cleared(
        store: &BlockStore,
        id: Uuid,
        client_id: ClientId,
        presence: HashMap<Uuid, Vec<u8>>,
        deliveries: &[(WatchIdentity, OutboundMessages)],
    ) {
        for presence_id in presence.into_keys() {
            let message = ServerMessage::Presence {
                id,
                client_id,
                presence_id,
                data: None,
            };
            Self::deliver(store, id, &message, deliveries).await;
        }
    }

    async fn deliver(
        store: &BlockStore,
        id: Uuid,
        message: &ServerMessage,
        deliveries: &[(WatchIdentity, OutboundMessages)],
    ) {
        for (identity, outbound) in deliveries {
            if store.access((*identity).into()).await.get(id).can_view() {
                outbound.send(message.clone());
            }
        }
    }

    async fn watch_references(
        &self,
        identity: Identity,
        list: BlockReferenceList,
        client_id: ClientId,
        outbound: OutboundMessages,
        last: Vec<BlockReference>,
    ) {
        self.reference_watchers
            .lock()
            .await
            .entry((identity.into(), list))
            .or_default()
            .insert(client_id, ReferenceWatch { outbound, last });
    }

    async fn unwatch_references(
        &self,
        identity: Identity,
        list: BlockReferenceList,
        client_id: ClientId,
    ) {
        let mut watchers = self.reference_watchers.lock().await;
        let key = (WatchIdentity::from(identity), list);
        if let Some(entries) = watchers.get_mut(&key) {
            entries.remove(&client_id);
            if entries.is_empty() {
                watchers.remove(&key);
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
        for (identity, list) in lists {
            let blocks = store.references(identity.into(), list).await;
            let mut watchers = self.reference_watchers.lock().await;
            let Some(entries) = watchers.get_mut(&(identity, list)) else {
                continue;
            };
            for watch in entries.values_mut() {
                if watch.last != blocks {
                    watch.last.clone_from(&blocks);
                    watch.outbound.send(ServerMessage::ReferencesUpdated {
                        list,
                        blocks: blocks.clone(),
                    });
                }
            }
        }
    }

    async fn broadcast(&self, store: &BlockStore, workspace_id: Uuid, message: ServerMessage) {
        let Some(id) = message.id() else {
            return;
        };
        let deliveries: Vec<_> = {
            let watchers = self.watchers.lock().await;
            watchers
                .get(&(workspace_id, id))
                .map(|entries| {
                    entries
                        .values()
                        .map(|watch| (watch.identity, watch.outbound.clone()))
                        .collect()
                })
                .unwrap_or_default()
        };
        for (identity, outbound) in deliveries {
            if store.access(identity.into()).await.get(id).can_view() {
                outbound.send(message.clone());
            }
        }
    }

    async fn broadcast_batch(
        &self,
        store: &BlockStore,
        workspace_id: Uuid,
        operations: Vec<BlockOperation>,
    ) {
        let mut deliveries: HashMap<
            ClientId,
            (WatchIdentity, OutboundMessages, Vec<BlockOperation>),
        > = HashMap::new();
        {
            let watchers = self.watchers.lock().await;
            for operation in operations {
                if let Some(entries) = watchers.get(&(workspace_id, operation.id)) {
                    for (&client_id, watch) in entries {
                        let delivery = deliveries.entry(client_id).or_insert_with(|| {
                            (watch.identity, watch.outbound.clone(), Vec::new())
                        });
                        delivery.2.push(operation.clone());
                    }
                }
            }
        }
        for (_, (identity, outbound, operations)) in deliveries {
            let access = store.access(identity.into()).await;
            let operations: Vec<_> = operations
                .into_iter()
                .filter(|operation| access.get(operation.id).can_view())
                .collect();
            if !operations.is_empty() {
                outbound.send(ServerMessage::BatchUpdated { operations });
            }
        }
    }
}

/// Per-block write locks, keyed by workspace and block id.
type BlockLocks = HashMap<(Uuid, Uuid), Arc<Mutex<()>>>;

struct BlockStore {
    database: Mutex<Connection>,
    locks: Mutex<BlockLocks>,
    dependencies: Mutex<HashMap<Uuid, DependencyState>>,
    allow_registration: bool,
}

impl BlockStore {
    #[cfg(test)]
    fn new(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).unwrap();
        let mut connection = Connection::open(root.join(DATABASE_FILE)).unwrap();
        initialize_database(&mut connection).unwrap();
        Self::from_connection(connection, ServerConfig::default()).unwrap()
    }

    async fn open_with_config(root: PathBuf, config: ServerConfig) -> Result<Self, ServerError> {
        let mut connection = Connection::open(root.join(DATABASE_FILE))?;
        initialize_database(&mut connection)?;
        Self::from_connection(connection, config).map_err(ServerError::from)
    }

    fn from_connection(connection: Connection, config: ServerConfig) -> Result<Self, StoreError> {
        let dependencies = load_dependencies(&connection)?;
        Ok(Self {
            database: Mutex::new(connection),
            locks: Mutex::new(HashMap::new()),
            dependencies: Mutex::new(dependencies),
            allow_registration: config.allow_registration,
        })
    }

    async fn lock_for(&self, workspace_id: Uuid, id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().await;
        Arc::clone(
            locks
                .entry((workspace_id, id))
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    async fn workspace_role(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<WorkspaceRole, ManagementStoreError> {
        let database = self.database.lock().await;
        require_membership(&database, account_id, workspace_id)
    }

    /// The permissions `identity` currently has across the whole workspace.
    async fn access(&self, identity: Identity) -> WorkspaceAccess {
        if identity.role == WorkspaceRole::Administrator {
            return WorkspaceAccess::Unrestricted;
        }
        let dependencies = self.dependencies.lock().await;
        let blocks = dependencies
            .get(&identity.workspace_id)
            .map(|workspace| workspace.access(identity.account_id))
            .unwrap_or_default();
        WorkspaceAccess::Blocks(blocks)
    }

    async fn register_account(
        &self,
        email: String,
        display_name: String,
        password: String,
    ) -> Result<(Account, String), ManagementStoreError> {
        let email = normalize_email(&email)?;
        let display_name = normalize_display_name(&display_name)?;
        let password_hash = hash_password(&password)?;
        let account = Account {
            id: Uuid::new_v4(),
            email,
            display_name,
        };
        let token = generate_token();
        let token_hash = hash_token(&token);
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        let result = transaction.execute(
            "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, ?4)",
            params![
                account.id.to_string(),
                &account.email,
                &account.display_name,
                &password_hash,
            ],
        );
        match result {
            Ok(_) => {}
            Err(error)
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
            {
                return Err(ManagementStoreError::EmailAlreadyRegistered);
            }
            Err(error) => return Err(error.into()),
        }
        transaction.execute(
            "INSERT INTO sessions (token_hash, account_id) VALUES (?1, ?2)",
            params![&token_hash, account.id.to_string()],
        )?;
        transaction.commit()?;
        Ok((account, token))
    }

    async fn login_account(
        &self,
        email: String,
        password: String,
    ) -> Result<(Account, String), ManagementStoreError> {
        let email = normalize_email(&email)?;
        let mut database = self.database.lock().await;
        let row = database
            .query_row(
                "SELECT id, email, display_name, password_hash FROM accounts WHERE email = ?1",
                [&email],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(ManagementStoreError::InvalidCredentials)?;
        if !verify_password(&password, &row.3) {
            return Err(ManagementStoreError::InvalidCredentials);
        }
        let account = Account {
            id: Uuid::parse_str(&row.0)
                .map_err(|error| ManagementStoreError::InvalidStorage(error.to_string()))?,
            email: row.1,
            display_name: row.2,
        };
        let token = generate_token();
        let token_hash = hash_token(&token);
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT INTO sessions (token_hash, account_id) VALUES (?1, ?2)",
            params![&token_hash, account.id.to_string()],
        )?;
        transaction.commit()?;
        Ok((account, token))
    }

    /// Resolves a session token to the account it authenticates, as proven by
    /// possession of the token rather than anything the client asserts.
    async fn resolve_token(&self, token: &str) -> Result<Uuid, ManagementStoreError> {
        let token_hash = hash_token(token);
        let database = self.database.lock().await;
        let account_id = database
            .query_row(
                "SELECT account_id FROM sessions WHERE token_hash = ?1",
                [&token_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(ManagementStoreError::InvalidToken)?;
        parse_management_uuid(&account_id)
    }

    /// Revokes a session token. Revoking a token that is already invalid is
    /// not an error: the caller's goal, that the token no longer works, is
    /// already true.
    async fn logout(&self, token: &str) -> Result<(), ManagementStoreError> {
        let token_hash = hash_token(token);
        let database = self.database.lock().await;
        database.execute("DELETE FROM sessions WHERE token_hash = ?1", [&token_hash])?;
        Ok(())
    }

    async fn list_workspaces(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<Workspace>, ManagementStoreError> {
        let database = self.database.lock().await;
        require_account(&database, account_id)?;
        let mut statement = database.prepare(
            "SELECT w.id, w.name, w.owner_id, m.role
             FROM workspace_memberships m
             JOIN workspaces w ON w.id = m.workspace_id
             WHERE m.account_id = ?1
             ORDER BY w.name COLLATE NOCASE, w.id",
        )?;
        let rows = statement.query_map([account_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, owner_id, role) = row?;
            Ok(Workspace {
                id: parse_management_uuid(&id)?,
                name,
                owner_id: parse_management_uuid(&owner_id)?,
                role: decode_workspace_role(&role)?,
            })
        })
        .collect()
    }

    async fn create_workspace(
        &self,
        account_id: Uuid,
        name: String,
    ) -> Result<Workspace, ManagementStoreError> {
        let name = normalize_display_name(&name)?;
        let workspace = Workspace {
            id: Uuid::new_v4(),
            name,
            owner_id: account_id,
            role: WorkspaceRole::Administrator,
        };
        let mut database = self.database.lock().await;
        require_account(&database, account_id)?;
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT INTO workspaces (id, name, owner_id) VALUES (?1, ?2, ?3)",
            params![
                workspace.id.to_string(),
                &workspace.name,
                workspace.owner_id.to_string()
            ],
        )?;
        transaction.execute(
            "INSERT INTO workspace_memberships (workspace_id, account_id, role)
             VALUES (?1, ?2, 'administrator')",
            params![workspace.id.to_string(), account_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(workspace)
    }

    async fn list_invitations(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<WorkspaceInvitation>, ManagementStoreError> {
        let database = self.database.lock().await;
        let email = account_email(&database, account_id)?;
        let mut statement = database.prepare(
            "SELECT i.id, i.workspace_id, w.name, i.email, i.role, i.invited_by
             FROM workspace_invitations i
             JOIN workspaces w ON w.id = i.workspace_id
             WHERE i.email = ?1
             ORDER BY w.name COLLATE NOCASE, i.id",
        )?;
        let rows = statement.query_map([email], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        rows.map(|row| {
            let (id, workspace_id, workspace_name, email, role, invited_by) = row?;
            Ok(WorkspaceInvitation {
                id: parse_management_uuid(&id)?,
                workspace_id: parse_management_uuid(&workspace_id)?,
                workspace_name,
                email,
                role: decode_workspace_role(&role)?,
                invited_by: parse_management_uuid(&invited_by)?,
            })
        })
        .collect()
    }

    async fn invite(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
        email: String,
        role: WorkspaceRole,
    ) -> Result<WorkspaceInvitation, ManagementStoreError> {
        let email = normalize_email(&email)?;
        let database = self.database.lock().await;
        require_administrator(&database, account_id, workspace_id)?;
        let workspace_name = database
            .query_row(
                "SELECT name FROM workspaces WHERE id = ?1",
                [workspace_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(ManagementStoreError::WorkspaceNotFound)?;
        let existing_account = database
            .query_row(
                "SELECT id FROM accounts WHERE email = ?1",
                [&email],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(existing_account) = existing_account {
            let membership = database
                .query_row(
                    "SELECT 1 FROM workspace_memberships
                     WHERE workspace_id = ?1 AND account_id = ?2",
                    params![workspace_id.to_string(), existing_account],
                    |_| Ok(()),
                )
                .optional()?;
            if membership.is_some() {
                return Err(ManagementStoreError::AccountAlreadyMember);
            }
        }
        let invitation = WorkspaceInvitation {
            id: Uuid::new_v4(),
            workspace_id,
            workspace_name,
            email,
            role,
            invited_by: account_id,
        };
        let result = database.execute(
            "INSERT INTO workspace_invitations
             (id, workspace_id, email, role, invited_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                invitation.id.to_string(),
                invitation.workspace_id.to_string(),
                &invitation.email,
                encode_workspace_role(invitation.role),
                invitation.invited_by.to_string()
            ],
        );
        match result {
            Ok(_) => Ok(invitation),
            Err(error)
                if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
            {
                Err(ManagementStoreError::InvitationAlreadyExists)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn respond_invitation(
        &self,
        account_id: Uuid,
        invitation_id: Uuid,
        accept: bool,
    ) -> Result<(), ManagementStoreError> {
        let mut database = self.database.lock().await;
        let email = account_email(&database, account_id)?;
        let invitation = database
            .query_row(
                "SELECT workspace_id, email, role FROM workspace_invitations WHERE id = ?1",
                [invitation_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(ManagementStoreError::InvitationNotFound)?;
        if invitation.1 != email {
            return Err(ManagementStoreError::PermissionDenied);
        }
        let transaction = database.transaction()?;
        if accept {
            transaction.execute(
                "INSERT INTO workspace_memberships (workspace_id, account_id, role)
                 VALUES (?1, ?2, ?3)",
                params![invitation.0, account_id.to_string(), invitation.2],
            )?;
        }
        transaction.execute(
            "DELETE FROM workspace_invitations WHERE id = ?1",
            [invitation_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    async fn create_block_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        block_type: Uuid,
        author: Uuid,
        data: Vec<u8>,
        properties: BTreeMap<Uuid, Vec<u8>>,
        dynamic_artifact: bool,
        references: Vec<Uuid>,
    ) -> Result<(), StoreError> {
        validate_properties(&properties)?;
        let references = normalize_ids(references);
        let mut dependencies = self.dependencies.lock().await;
        let workspace = dependencies.entry(workspace_id).or_default();
        if workspace.blocks.contains_key(&id) {
            return Err(StoreError::BlockAlreadyExists);
        }
        if references
            .iter()
            .any(|reference| *reference != id && !workspace.blocks.contains_key(reference))
        {
            return Err(StoreError::ReferencedBlockNotFound);
        }
        let mut updated = workspace.clone();
        updated.blocks.insert(
            id,
            DependencyBlock {
                block_type,
                author,
                properties,
                dynamic_artifact,
                parent: BlockParent::Orphaned,
                references,
                backrefs: Vec::new(),
                grants: HashMap::new(),
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
                workspace_id, id, block_type, author, snapshot, snapshot_seq,
                dynamic_artifact, parent_kind, parent_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, 0, NULL)",
            params![
                workspace_id.to_string(),
                id.to_string(),
                block_type.to_string(),
                author.to_string(),
                data,
                dynamic_artifact,
            ],
        )?;
        persist_dependencies(&transaction, workspace_id, &updated)?;
        transaction.commit()?;
        *workspace = updated;
        Ok(())
    }

    async fn update_block_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        seq: Option<u64>,
        operation_id: Uuid,
        author: Uuid,
        operation: Vec<u8>,
        properties: BTreeMap<Uuid, Vec<u8>>,
        dynamic_artifact: bool,
        references: ReferenceDelta,
    ) -> Result<UpdateOutcome, StoreError> {
        validate_properties(&properties)?;
        let (exists, records) = {
            let database = self.database.lock().await;
            let exists = database
                .query_row(
                    "SELECT 1 FROM blocks WHERE workspace_id = ?1 AND id = ?2",
                    params![workspace_id.to_string(), id.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            let records = exists
                .then(|| read_operations(&database, workspace_id, id))
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
                let properties = dependencies
                    .get(&workspace_id)
                    .ok_or(StoreError::BlockNotFound)?
                    .blocks
                    .get(&id)
                    .ok_or(StoreError::BlockNotFound)?
                    .properties
                    .clone();
                return Ok(UpdateOutcome::Duplicate(existing.clone(), properties));
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
        let workspace = dependencies.entry(workspace_id).or_default();
        let mut updated = workspace.clone();
        updated.apply_references(id, &record.references)?;
        let block = updated
            .blocks
            .get_mut(&id)
            .ok_or(StoreError::BlockNotFound)?;
        block.properties = properties;
        block.dynamic_artifact = dynamic_artifact;
        let properties = block.properties.clone();
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        transaction.execute(
            "INSERT INTO operations (
                workspace_id, block_id, seq, operation_id, author, operation, reference_delta
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                workspace_id.to_string(),
                id.to_string(),
                u64_to_i64(record.seq)?,
                record.operation_id.to_string(),
                record.author.to_string(),
                &record.operation,
                serde_json::to_string(&record.references)?,
            ],
        )?;
        persist_dependencies(&transaction, workspace_id, &updated)?;
        transaction.commit()?;
        *workspace = updated;
        Ok(UpdateOutcome::Inserted(record, properties))
    }

    async fn read_block_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<BlockRead, StoreError> {
        let database = self.database.lock().await;
        let block = database
            .query_row(
                "SELECT block_type, snapshot, snapshot_seq FROM blocks
                 WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id.to_string(), id.to_string()],
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
        let operations = read_operations(&database, workspace_id, id)?
            .into_iter()
            .filter(|record| record.seq > snapshot_seq)
            .collect();
        drop(database);
        let dependencies = self.dependencies.lock().await;
        let dependency = dependencies
            .get(&workspace_id)
            .ok_or(StoreError::BlockNotFound)?
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
            properties: dependency.properties.clone(),
        })
    }

    async fn set_parent_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        parent: BlockParent,
    ) -> Result<(), StoreError> {
        let mut dependencies = self.dependencies.lock().await;
        let workspace = dependencies.entry(workspace_id).or_default();
        let mut updated = workspace.clone();
        updated.set_parent(id, parent)?;
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        persist_dependencies(&transaction, workspace_id, &updated)?;
        transaction.commit()?;
        *workspace = updated;
        Ok(())
    }

    async fn set_property_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        key: Uuid,
        value: Vec<u8>,
    ) -> Result<Vec<u8>, StoreError> {
        validate_property_value(&value)?;
        let mut dependencies = self.dependencies.lock().await;
        let workspace = dependencies.entry(workspace_id).or_default();
        let mut updated = workspace.clone();
        let block = updated
            .blocks
            .get_mut(&id)
            .ok_or(StoreError::BlockNotFound)?;
        block.properties.insert(key, value.clone());
        let mut database = self.database.lock().await;
        let transaction = database.transaction()?;
        persist_dependencies(&transaction, workspace_id, &updated)?;
        transaction.commit()?;
        *workspace = updated;
        Ok(value)
    }

    async fn references(
        &self,
        identity: Identity,
        list: BlockReferenceList,
    ) -> Vec<BlockReference> {
        let access = self.access(identity).await;
        let dependencies = self.dependencies.lock().await;
        let Some(dependencies) = dependencies.get(&identity.workspace_id) else {
            return Vec::new();
        };
        // Listing the neighbours of a block the account cannot see would leak
        // its shape, so the whole listing collapses to nothing.
        let subject = match list {
            BlockReferenceList::Roots | BlockReferenceList::Orphans => None,
            BlockReferenceList::Parents(id)
            | BlockReferenceList::References(id)
            | BlockReferenceList::Backrefs(id) => Some(id),
        };
        if subject.is_some_and(|id| !access.get(id).can_know_exists()) {
            return Vec::new();
        }
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
                let listed = access.get(id);
                if !listed.can_know_exists() {
                    return None;
                }
                dependencies.blocks.get(&id).map(|block| BlockReference {
                    id,
                    block_type: block.block_type,
                    author: block.author,
                    properties: block.properties.clone(),
                    parent: block.parent,
                    references: block
                        .references
                        .iter()
                        .filter(|reference| access.get(**reference).can_know_exists())
                        .count(),
                    dynamic_artifact: block.dynamic_artifact,
                    access: listed,
                })
            })
            .collect()
    }

    /// Every workspace member alongside the access they have to `id`, so a
    /// client managing sharing can show the full picture in one round trip.
    async fn block_access_entries(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Vec<BlockAccessEntry>, StoreError> {
        let members = {
            let database = self.database.lock().await;
            let mut statement = database.prepare(
                "SELECT a.id, a.email, a.display_name, m.role
                 FROM workspace_memberships m
                 JOIN accounts a ON a.id = m.account_id
                 WHERE m.workspace_id = ?1
                 ORDER BY a.display_name COLLATE NOCASE, a.id",
            )?;
            let rows = statement.query_map([workspace_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let dependencies = self.dependencies.lock().await;
        let workspace = dependencies
            .get(&workspace_id)
            .ok_or(StoreError::BlockNotFound)?;
        if !workspace.blocks.contains_key(&id) {
            return Err(StoreError::BlockNotFound);
        }
        members
            .into_iter()
            .map(|(account_id, email, display_name, role)| {
                let account_id = parse_uuid(&account_id)?;
                let role = decode_workspace_role(&role)
                    .map_err(|error| StoreError::InvalidStorage(format!("{error:?}")))?;
                let effective = if role == WorkspaceRole::Administrator {
                    BlockAccess::Edit
                } else {
                    workspace
                        .access(account_id)
                        .get(&id)
                        .copied()
                        .unwrap_or(BlockAccess::None)
                };
                Ok(BlockAccessEntry {
                    account: Account {
                        id: account_id,
                        email,
                        display_name,
                    },
                    role,
                    granted: workspace.blocks[&id].grants.get(&account_id).copied(),
                    effective,
                })
            })
            .collect()
    }

    async fn set_block_access_unlocked(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        account_id: Uuid,
        access: BlockAccess,
    ) -> Result<(), StoreError> {
        let mut dependencies = self.dependencies.lock().await;
        let workspace = dependencies.entry(workspace_id).or_default();
        if !workspace.blocks.contains_key(&id) {
            return Err(StoreError::BlockNotFound);
        }
        let mut database = self.database.lock().await;
        let is_member = database
            .query_row(
                "SELECT role FROM workspace_memberships
                 WHERE workspace_id = ?1 AND account_id = ?2",
                params![workspace_id.to_string(), account_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StoreError::AccountNotFound)?;
        // Administrators already reach every block, so recording a grant for one
        // would be a permission the workspace role silently overrides.
        if decode_workspace_role(&is_member)
            .map_err(|error| StoreError::InvalidStorage(format!("{error:?}")))?
            == WorkspaceRole::Administrator
        {
            return Err(StoreError::AdministratorAccessIsImplicit);
        }

        let transaction = database.transaction()?;
        if access == BlockAccess::None {
            transaction.execute(
                "DELETE FROM block_access
                 WHERE workspace_id = ?1 AND block_id = ?2 AND account_id = ?3",
                params![
                    workspace_id.to_string(),
                    id.to_string(),
                    account_id.to_string()
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO block_access (workspace_id, block_id, account_id, access)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(workspace_id, block_id, account_id) DO UPDATE SET
                    access = excluded.access",
                params![
                    workspace_id.to_string(),
                    id.to_string(),
                    account_id.to_string(),
                    encode_block_access(access),
                ],
            )?;
        }
        transaction.commit()?;

        let grants = &mut workspace.blocks.get_mut(&id).unwrap().grants;
        if access == BlockAccess::None {
            grants.remove(&account_id);
        } else {
            grants.insert(account_id, access);
        }
        Ok(())
    }
}

/// A resolved view of what one account may do in a workspace.
enum WorkspaceAccess {
    /// Administrators bypass per-block permissions entirely.
    Unrestricted,
    Blocks(HashMap<Uuid, BlockAccess>),
}

impl WorkspaceAccess {
    fn get(&self, id: Uuid) -> BlockAccess {
        match self {
            Self::Unrestricted => BlockAccess::Edit,
            Self::Blocks(blocks) => blocks.get(&id).copied().unwrap_or(BlockAccess::None),
        }
    }
}

enum UpdateOutcome {
    Inserted(OperationRecord, BTreeMap<Uuid, Vec<u8>>),
    Duplicate(OperationRecord, BTreeMap<Uuid, Vec<u8>>),
}

struct BlockRead {
    block_type: Uuid,
    author: Uuid,
    snapshot: Vec<u8>,
    snapshot_seq: u64,
    operations: Vec<OperationRecord>,
    parent: BlockParent,
    properties: BTreeMap<Uuid, Vec<u8>>,
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

    /// Resolves what `account_id` may do with every block in the workspace.
    ///
    /// Access starts from the blocks the account authored or was granted
    /// directly, then spreads along three rules: view and edit flow down into
    /// owned children, a viewable block reveals the existence of the blocks it
    /// references, and knowing a block exists reveals its ancestors.
    fn access(&self, account_id: Uuid) -> HashMap<Uuid, BlockAccess> {
        let direct: HashMap<Uuid, BlockAccess> = self
            .blocks
            .iter()
            .map(|(&id, block)| {
                let granted = block.grants.get(&account_id).copied();
                let authored = (block.author == account_id).then_some(BlockAccess::Edit);
                (id, granted.max(authored).unwrap_or(BlockAccess::None))
            })
            .collect();

        let mut access: HashMap<Uuid, BlockAccess> = self
            .blocks
            .keys()
            .map(|&id| (id, self.inherited_access(&direct, id)))
            .collect();

        // A block you can open reveals that the blocks it points at exist, but
        // only for plain references: owned children already inherited above.
        let referenced: Vec<Uuid> = self
            .blocks
            .iter()
            .filter(|(id, _)| access[*id].can_view())
            .flat_map(|(&id, block)| {
                block
                    .references
                    .iter()
                    .copied()
                    .filter(move |reference| !self.is_owned_child(id, *reference))
            })
            .collect();
        for id in referenced {
            let entry = access.entry(id).or_insert(BlockAccess::None);
            *entry = (*entry).max(BlockAccess::KnowExists);
        }

        // Anything known to exist makes its whole ancestor chain known too,
        // otherwise it could never be located in a listing.
        let known: Vec<Uuid> = access
            .iter()
            .filter(|(_, level)| level.can_know_exists())
            .map(|(&id, _)| id)
            .collect();
        for id in known {
            let mut seen = HashSet::from([id]);
            let mut current = self.blocks.get(&id).map(|block| block.parent);
            while let Some(BlockParent::Uuid(parent)) = current {
                if !seen.insert(parent) {
                    break;
                }
                let entry = access.entry(parent).or_insert(BlockAccess::None);
                *entry = (*entry).max(BlockAccess::KnowExists);
                current = self.blocks.get(&parent).map(|block| block.parent);
            }
        }

        access
    }

    /// The strongest view or edit permission reaching `id` from itself or any
    /// block that owns it, combined with any weaker direct grant on `id`.
    fn inherited_access(&self, direct: &HashMap<Uuid, BlockAccess>, id: Uuid) -> BlockAccess {
        let mut access = direct.get(&id).copied().unwrap_or(BlockAccess::None);
        let mut seen = HashSet::from([id]);
        let mut current = self.blocks.get(&id).map(|block| block.parent);
        while let Some(BlockParent::Uuid(parent)) = current {
            if !seen.insert(parent) {
                break;
            }
            let inherited = direct.get(&parent).copied().unwrap_or(BlockAccess::None);
            if inherited.can_view() {
                access = access.max(inherited);
            }
            current = self.blocks.get(&parent).map(|block| block.parent);
        }
        access
    }

    fn is_owned_child(&self, parent: Uuid, child: Uuid) -> bool {
        self.blocks
            .get(&child)
            .is_some_and(|block| block.parent == BlockParent::Uuid(parent))
    }
}

#[derive(Clone)]
struct DependencyBlock {
    block_type: Uuid,
    author: Uuid,
    /// Every property key and value recorded against the block. Properties
    /// are opaque to the server; only the client interprets them.
    properties: BTreeMap<Uuid, Vec<u8>>,
    /// Whether the block is generated from another one. The server never reads
    /// what a block holds, so its author reports this alongside the name.
    dynamic_artifact: bool,
    parent: BlockParent,
    references: Vec<Uuid>,
    backrefs: Vec<Uuid>,
    /// Permissions recorded directly against this block, by account.
    grants: HashMap<Uuid, BlockAccess>,
}

fn validate_property_value(value: &[u8]) -> Result<(), StoreError> {
    if value.len() > MAX_PROPERTY_VALUE_BYTES {
        return Err(StoreError::InvalidMessage(format!(
            "property value exceeds {MAX_PROPERTY_VALUE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_properties(properties: &BTreeMap<Uuid, Vec<u8>>) -> Result<(), StoreError> {
    properties
        .values()
        .try_for_each(|value| validate_property_value(value))
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

/// How long an account's display name may be. Unrelated to block properties;
/// just a reasonable length cap for a person's name.
const MAX_DISPLAY_NAME_BYTES: usize = 128;

fn normalize_display_name(name: &str) -> Result<String, ManagementStoreError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(ManagementStoreError::InvalidName);
    }
    Ok(name.to_owned())
}

/// Hashes a password for storage. Argon2's own salt and work-factor defaults
/// are used, encoded into the returned PHC string alongside the hash so
/// `verify_password` needs nothing else to check it later.
fn hash_password(password: &str) -> Result<String, ManagementStoreError> {
    if password.len() < MIN_PASSWORD_BYTES {
        return Err(ManagementStoreError::InvalidPassword);
    }
    let salt = SaltString::generate(&mut PasswordHashOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| ManagementStoreError::InvalidStorage(error.to_string()))
}

/// Checks a password against a hash produced by `hash_password`. Any error
/// parsing the stored hash is treated as a mismatch rather than propagated,
/// since a garbled `password_hash` should never be able to authenticate.
fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// A fresh, high-entropy session token. Tokens are bearer credentials, so
/// their strength comes entirely from this randomness rather than from a
/// deliberately slow hash like passwords use.
fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    to_hex(&bytes)
}

/// The value actually stored for a session token: only its hash, so a
/// database leak does not hand out working credentials.
fn hash_token(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require_account(database: &Connection, account_id: Uuid) -> Result<(), ManagementStoreError> {
    database
        .query_row(
            "SELECT 1 FROM accounts WHERE id = ?1",
            [account_id.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .ok_or(ManagementStoreError::AccountNotFound)
}

fn account_email(database: &Connection, account_id: Uuid) -> Result<String, ManagementStoreError> {
    database
        .query_row(
            "SELECT email FROM accounts WHERE id = ?1",
            [account_id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ManagementStoreError::AccountNotFound)
}

fn require_membership(
    database: &Connection,
    account_id: Uuid,
    workspace_id: Uuid,
) -> Result<WorkspaceRole, ManagementStoreError> {
    require_account(database, account_id)?;
    let workspace_exists = database
        .query_row(
            "SELECT 1 FROM workspaces WHERE id = ?1",
            [workspace_id.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !workspace_exists {
        return Err(ManagementStoreError::WorkspaceNotFound);
    }
    let role = database
        .query_row(
            "SELECT role FROM workspace_memberships
             WHERE workspace_id = ?1 AND account_id = ?2",
            params![workspace_id.to_string(), account_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(ManagementStoreError::PermissionDenied)?;
    decode_workspace_role(&role)
}

fn require_administrator(
    database: &Connection,
    account_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), ManagementStoreError> {
    match require_membership(database, account_id, workspace_id)? {
        WorkspaceRole::Administrator => Ok(()),
        WorkspaceRole::Editor => Err(ManagementStoreError::PermissionDenied),
    }
}

fn parse_management_uuid(value: &str) -> Result<Uuid, ManagementStoreError> {
    Uuid::parse_str(value).map_err(|error| ManagementStoreError::InvalidStorage(error.to_string()))
}

fn decode_workspace_role(value: &str) -> Result<WorkspaceRole, ManagementStoreError> {
    match value {
        "administrator" => Ok(WorkspaceRole::Administrator),
        "editor" => Ok(WorkspaceRole::Editor),
        _ => Err(ManagementStoreError::InvalidStorage(format!(
            "invalid workspace role {value}"
        ))),
    }
}

fn encode_workspace_role(role: WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Administrator => "administrator",
        WorkspaceRole::Editor => "editor",
    }
}

fn encode_block_access(access: BlockAccess) -> &'static str {
    match access {
        BlockAccess::None => "none",
        BlockAccess::KnowExists => "know_exists",
        BlockAccess::View => "view",
        BlockAccess::Edit => "edit",
    }
}

fn decode_block_access(value: &str) -> Result<BlockAccess, StoreError> {
    match value {
        "know_exists" => Ok(BlockAccess::KnowExists),
        "view" => Ok(BlockAccess::View),
        "edit" => Ok(BlockAccess::Edit),
        _ => Err(StoreError::InvalidStorage(format!(
            "invalid block access {value}"
        ))),
    }
}

fn normalize_ids(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

fn initialize_database(connection: &mut Connection) -> Result<(), StoreError> {
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    for (table, expected) in EXPECTED_TABLES {
        let columns = table_columns(connection, table)?;
        // Tables that are not there yet are created below.
        if !columns.is_empty()
            && !columns
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied())
        {
            return Err(StoreError::InvalidStorage(format!(
                "the {table} table has the columns [{}] but this server expects [{}]. \
                 Stored blocks are never migrated, so the database has to be deleted \
                 before this server can open it.",
                columns.join(", "),
                expected.join(", "),
            )));
        }
    }
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS blocks (
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            id              TEXT NOT NULL,
            block_type      TEXT NOT NULL,
            author          TEXT NOT NULL,
            snapshot        BLOB NOT NULL,
            snapshot_seq    INTEGER NOT NULL CHECK (snapshot_seq >= 0),
            dynamic_artifact INTEGER NOT NULL CHECK (dynamic_artifact IN (0, 1)),
            parent_kind     INTEGER NOT NULL CHECK (parent_kind IN (0, 1, 2)),
            parent_id       TEXT,
            CHECK (
                (parent_kind = 2 AND parent_id IS NOT NULL)
                OR (parent_kind != 2 AND parent_id IS NULL)
            ),
            PRIMARY KEY (workspace_id, id)
        );

        CREATE TABLE IF NOT EXISTS block_properties (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            key             TEXT NOT NULL,
            value           BLOB NOT NULL,
            PRIMARY KEY (workspace_id, block_id, key),
            FOREIGN KEY (workspace_id, block_id)
                REFERENCES blocks(workspace_id, id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS block_references (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            position        INTEGER NOT NULL CHECK (position >= 0),
            reference_id    TEXT NOT NULL,
            PRIMARY KEY (workspace_id, block_id, position),
            UNIQUE (workspace_id, block_id, reference_id),
            FOREIGN KEY (workspace_id, block_id)
                REFERENCES blocks(workspace_id, id) ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, reference_id)
                REFERENCES blocks(workspace_id, id)
        );

        CREATE TABLE IF NOT EXISTS operations (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            seq             INTEGER NOT NULL CHECK (seq > 0),
            operation_id    TEXT NOT NULL,
            author          TEXT NOT NULL,
            operation       BLOB NOT NULL,
            reference_delta TEXT NOT NULL,
            PRIMARY KEY (workspace_id, block_id, seq),
            UNIQUE (workspace_id, block_id, operation_id),
            FOREIGN KEY (workspace_id, block_id)
                REFERENCES blocks(workspace_id, id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS accounts (
            id              TEXT PRIMARY KEY,
            email           TEXT NOT NULL UNIQUE,
            display_name    TEXT NOT NULL,
            password_hash   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            token_hash      TEXT PRIMARY KEY,
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS workspaces (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            owner_id        TEXT NOT NULL REFERENCES accounts(id)
        );

        CREATE TABLE IF NOT EXISTS workspace_memberships (
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            role            TEXT NOT NULL CHECK (role IN ('administrator', 'editor')),
            PRIMARY KEY (workspace_id, account_id)
        );

        CREATE TABLE IF NOT EXISTS workspace_invitations (
            id              TEXT PRIMARY KEY,
            workspace_id    TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
            email           TEXT NOT NULL,
            role            TEXT NOT NULL CHECK (role IN ('administrator', 'editor')),
            invited_by      TEXT NOT NULL REFERENCES accounts(id),
            UNIQUE (workspace_id, email)
        );

        CREATE TABLE IF NOT EXISTS block_access (
            workspace_id    TEXT NOT NULL,
            block_id        TEXT NOT NULL,
            account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
            access          TEXT NOT NULL CHECK (access IN ('know_exists', 'view', 'edit')),
            PRIMARY KEY (workspace_id, block_id, account_id),
            FOREIGN KEY (workspace_id, block_id)
                REFERENCES blocks(workspace_id, id) ON DELETE CASCADE
        );
        ",
    )?;
    Ok(())
}

/// The columns every table is expected to have, in the order they are declared
/// above. A database that disagrees was written by a different version of the
/// schema, and nothing here migrates it, so the server refuses to open it
/// rather than deciding on its own what to throw away.
const EXPECTED_TABLES: &[(&str, &[&str])] = &[
    (
        "blocks",
        &[
            "workspace_id",
            "id",
            "block_type",
            "author",
            "snapshot",
            "snapshot_seq",
            "dynamic_artifact",
            "parent_kind",
            "parent_id",
        ],
    ),
    (
        "block_properties",
        &["workspace_id", "block_id", "key", "value"],
    ),
    (
        "block_references",
        &["workspace_id", "block_id", "position", "reference_id"],
    ),
    (
        "operations",
        &[
            "workspace_id",
            "block_id",
            "seq",
            "operation_id",
            "author",
            "operation",
            "reference_delta",
        ],
    ),
    (
        "accounts",
        &["id", "email", "display_name", "password_hash"],
    ),
    ("sessions", &["token_hash", "account_id"]),
    ("workspaces", &["id", "name", "owner_id"]),
    (
        "workspace_memberships",
        &["workspace_id", "account_id", "role"],
    ),
    (
        "workspace_invitations",
        &["id", "workspace_id", "email", "role", "invited_by"],
    ),
    (
        "block_access",
        &["workspace_id", "block_id", "account_id", "access"],
    ),
];

/// The columns `table` has, or nothing at all when it does not exist yet.
fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, StoreError> {
    // A table name cannot be bound as a parameter, and these names are ours.
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

#[derive(Debug)]
enum ManagementStoreError {
    AccountAlreadyMember,
    AccountNotFound,
    EmailAlreadyRegistered,
    InvalidCredentials,
    InvalidEmail,
    InvalidName,
    InvalidPassword,
    InvalidToken,
    InvitationAlreadyExists,
    InvitationNotFound,
    InvalidStorage(String),
    PermissionDenied,
    RegistrationDisabled,
    Sqlite(rusqlite::Error),
    WorkspaceNotFound,
}

impl ManagementStoreError {
    fn to_response(&self, request_id: Uuid) -> ManagementServerMessage {
        let (code, message) = match self {
            Self::AccountAlreadyMember => (
                ManagementErrorCode::AccountAlreadyMember,
                "account is already a workspace member".into(),
            ),
            Self::AccountNotFound => (
                ManagementErrorCode::AccountNotFound,
                "account not found".into(),
            ),
            Self::EmailAlreadyRegistered => (
                ManagementErrorCode::EmailAlreadyRegistered,
                "email is already registered".into(),
            ),
            Self::InvalidCredentials => (
                ManagementErrorCode::InvalidCredentials,
                "incorrect email or password".into(),
            ),
            Self::InvalidEmail => (
                ManagementErrorCode::InvalidEmail,
                "invalid email address".into(),
            ),
            Self::InvalidName => (
                ManagementErrorCode::InvalidName,
                "invalid display name".into(),
            ),
            Self::InvalidPassword => (
                ManagementErrorCode::InvalidPassword,
                format!("password must be at least {MIN_PASSWORD_BYTES} bytes"),
            ),
            Self::InvalidToken => (
                ManagementErrorCode::InvalidToken,
                "session token is invalid or has been revoked".into(),
            ),
            Self::InvitationAlreadyExists => (
                ManagementErrorCode::InvitationAlreadyExists,
                "an invitation already exists for this email".into(),
            ),
            Self::InvitationNotFound => (
                ManagementErrorCode::InvitationNotFound,
                "invitation not found".into(),
            ),
            Self::InvalidStorage(message) => (ManagementErrorCode::StorageError, message.clone()),
            Self::PermissionDenied => (
                ManagementErrorCode::PermissionDenied,
                "permission denied".into(),
            ),
            Self::RegistrationDisabled => (
                ManagementErrorCode::RegistrationDisabled,
                "this server does not accept new registrations".into(),
            ),
            Self::Sqlite(error) => (ManagementErrorCode::StorageError, error.to_string()),
            Self::WorkspaceNotFound => (
                ManagementErrorCode::WorkspaceNotFound,
                "workspace not found".into(),
            ),
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

fn load_dependencies(
    connection: &Connection,
) -> Result<HashMap<Uuid, DependencyState>, StoreError> {
    let mut states: HashMap<Uuid, DependencyState> = HashMap::new();
    {
        let mut statement = connection.prepare(
            "SELECT workspace_id, id, block_type, author, dynamic_artifact,
                    parent_kind, parent_id
             FROM blocks ORDER BY workspace_id, rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        for row in rows {
            let (workspace_id, id, block_type, author, dynamic_artifact, parent_kind, parent_id) =
                row?;
            let parent = decode_parent(parent_kind, parent_id)?;
            states
                .entry(parse_uuid(&workspace_id)?)
                .or_default()
                .blocks
                .insert(
                    parse_uuid(&id)?,
                    DependencyBlock {
                        block_type: parse_uuid(&block_type)?,
                        author: parse_uuid(&author)?,
                        properties: BTreeMap::new(),
                        dynamic_artifact,
                        parent,
                        references: Vec::new(),
                        backrefs: Vec::new(),
                        grants: HashMap::new(),
                    },
                );
        }
    }

    {
        let mut statement = connection
            .prepare("SELECT workspace_id, block_id, key, value FROM block_properties")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?;
        for row in rows {
            let (workspace_id, block_id, key, value) = row?;
            let block_id = parse_uuid(&block_id)?;
            states
                .get_mut(&parse_uuid(&workspace_id)?)
                .and_then(|state| state.blocks.get_mut(&block_id))
                .ok_or_else(|| StoreError::InvalidStorage("property block is missing".into()))?
                .properties
                .insert(parse_uuid(&key)?, value);
        }
    }

    {
        let mut statement = connection
            .prepare("SELECT workspace_id, block_id, account_id, access FROM block_access")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (workspace_id, block_id, account_id, access) = row?;
            let block_id = parse_uuid(&block_id)?;
            states
                .get_mut(&parse_uuid(&workspace_id)?)
                .and_then(|state| state.blocks.get_mut(&block_id))
                .ok_or_else(|| StoreError::InvalidStorage("grant block is missing".into()))?
                .grants
                .insert(parse_uuid(&account_id)?, decode_block_access(&access)?);
        }
    }

    let mut statement = connection.prepare(
        "SELECT workspace_id, block_id, reference_id
         FROM block_references ORDER BY workspace_id, block_id, position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (workspace_id, block_id, reference_id) = row?;
        let workspace_id = parse_uuid(&workspace_id)?;
        let block_id = parse_uuid(&block_id)?;
        let reference_id = parse_uuid(&reference_id)?;
        states
            .get_mut(&workspace_id)
            .ok_or_else(|| StoreError::InvalidStorage("reference workspace is missing".into()))?
            .blocks
            .get_mut(&block_id)
            .ok_or_else(|| StoreError::InvalidStorage("reference source is missing".into()))?
            .references
            .push(reference_id);
    }

    for state in states.values_mut() {
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
    }
    Ok(states)
}

fn persist_dependencies(
    transaction: &Transaction<'_>,
    workspace_id: Uuid,
    dependencies: &DependencyState,
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM block_references WHERE workspace_id = ?1",
        [workspace_id.to_string()],
    )?;
    transaction.execute(
        "DELETE FROM block_properties WHERE workspace_id = ?1",
        [workspace_id.to_string()],
    )?;
    for (&id, block) in &dependencies.blocks {
        let (parent_kind, parent_id) = encode_parent(block.parent);
        let updated = transaction.execute(
            "UPDATE blocks
             SET dynamic_artifact = ?3, parent_kind = ?4, parent_id = ?5
             WHERE workspace_id = ?1 AND id = ?2",
            params![
                workspace_id.to_string(),
                id.to_string(),
                block.dynamic_artifact,
                parent_kind,
                parent_id,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidStorage(
                "dependency metadata references a missing block".into(),
            ));
        }
        for (key, value) in &block.properties {
            transaction.execute(
                "INSERT INTO block_properties (workspace_id, block_id, key, value)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    workspace_id.to_string(),
                    id.to_string(),
                    key.to_string(),
                    value
                ],
            )?;
        }
        for (position, reference) in block.references.iter().enumerate() {
            transaction.execute(
                "INSERT INTO block_references (workspace_id, block_id, position, reference_id)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    workspace_id.to_string(),
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

fn read_operations(
    connection: &Connection,
    workspace_id: Uuid,
    id: Uuid,
) -> Result<Vec<OperationRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT seq, operation_id, author, operation, reference_delta
         FROM operations WHERE workspace_id = ?1 AND block_id = ?2 ORDER BY seq",
    )?;
    let rows = statement.query_map(params![workspace_id.to_string(), id.to_string()], |row| {
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
    AccountNotFound,
    AdministratorAccessIsImplicit,
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
            Self::AccountNotFound => ErrorCode::AccountNotFound,
            Self::AdministratorAccessIsImplicit => ErrorCode::PermissionDenied,
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
            Self::AccountNotFound => write!(formatter, "account is not a workspace member"),
            Self::AdministratorAccessIsImplicit => {
                write!(
                    formatter,
                    "administrators already have access to every block"
                )
            }
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
    InvalidHandshake,
    InvalidRequest(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
    Storage(String),
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandshake => write!(formatter, "invalid block connection identity"),
            Self::InvalidRequest(message) => write!(formatter, "invalid request: {message}"),
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
