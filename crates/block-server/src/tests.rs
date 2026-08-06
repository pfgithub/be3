use super::*;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

mod a_database_from_another_schema_is_refused;
mod access_flows_down_to_owned_children_and_up_to_parents;
mod account_login_is_case_insensitive;
mod account_registration_rejects_duplicates;
mod account_state_survives_a_server_restart;
mod administrators_reach_every_block_without_grants;
mod batch_is_acknowledged_before_watch_notifications;
mod batch_updates_apply_reference_deltas_in_request_order;
mod block_access_survives_a_server_restart;
mod block_connections_require_a_valid_token;
mod block_connections_require_workspace_membership;
mod dependency_state_survives_a_server_restart;
mod editors_only_reach_blocks_they_authored_or_were_granted;
mod explicit_sequences_cannot_be_applied_out_of_order;
mod listings_report_whether_a_block_is_a_dynamic_artifact;
mod lists_backrefs_with_relationship_metadata;
mod lists_every_root_block_in_uuid_order;
mod lists_parents;
mod login_rejects_incorrect_passwords;
mod login_rejects_unknown_accounts;
mod logout_revokes_the_session_token;
mod management_answers_a_cors_preflight;
mod merges_reference_deltas_from_concurrent_clients;
mod missing_references_reject_creates_and_do_not_commit_updates;
mod omitted_sequences_are_assigned_by_the_server;
mod operation_ids_are_idempotent_and_conflicts_are_rejected;
mod parent_watch_updates;
mod pending_invitation_can_be_accepted_after_registration;
mod pending_invitation_can_be_declined;
mod presence_can_be_cleared_explicitly;
mod presence_is_broadcast_to_other_watchers_but_not_the_poster;
mod presence_is_cleared_when_a_client_disconnects;
mod presence_is_cleared_when_a_client_unwatches;
mod presence_replays_existing_values_to_a_new_watcher;
mod presence_requires_watching_the_block;
mod preserves_reference_order;
mod property_values_over_the_size_limit_are_rejected;
mod read_returns_parent;
mod reads_replay_contiguous_operation_records;
mod reference_watch_updates_when_a_listed_blocks_parent_changes;
mod reference_watch_updates_when_a_listed_blocks_reference_count_changes;
mod registration_can_be_disabled;
mod rejects_missing_parent_references_and_parent_cycles;
mod removing_a_parent_reference_orphans_the_child_without_restoring_it_on_readd;
mod reparents_without_changing_either_parents_references;
mod sequence_errors_include_the_expected_sequence;
mod shared_protocol_round_trips_over_websocket;
mod sharing_requires_edit_access_to_the_block;
mod watches_reference_changes_until_unwatched;
mod workspace_invites_require_administrator_membership;
mod workspace_state_survives_a_server_restart;
mod workspaces_isolate_identical_block_ids_and_notifications;
mod workspaces_start_empty_and_creation_adds_the_owner;

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
    token: &str,
    workspace_id: Uuid,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let request = format!("{url}/?token={token}&workspace={workspace_id}")
        .into_client_request()
        .unwrap();
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

/// A password meeting the server's minimum length, used by every test account
/// since none of these tests are about password strength.
const TEST_PASSWORD: &str = "correct horse battery staple";

mod support {
    #![allow(dead_code)]

    use std::{
        collections::BTreeMap,
        ops::Deref,
        path::{Path, PathBuf},
    };

    use block::{
        Account, BlockAccess, BlockAccessEntry, BlockParent, BlockReference, BlockReferenceList,
        ClientMessage, ManagementClientMessage, ManagementServerMessage, ReferenceDelta,
        ServerMessage, Workspace, WorkspaceRole,
    };
    use futures_util::{SinkExt, StreamExt};
    use tokio::{fs, net::TcpListener, task::JoinHandle};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message},
        MaybeTlsStream, WebSocketStream,
    };
    use uuid::Uuid;

    use super::TEST_PASSWORD;

    pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    /// An account plus the session token `register` obtained for it. Most call
    /// sites only care about the account fields, so this derefs to `Account`.
    pub struct TestAccount {
        pub account: Account,
        pub token: String,
    }

    impl Deref for TestAccount {
        type Target = Account;

        fn deref(&self) -> &Account {
            &self.account
        }
    }

    /// The HTTP endpoint management commands are sent to, kept separate from the
    /// websocket the block protocol runs on.
    pub struct Management {
        url: String,
    }

    impl Management {
        pub fn new(url: &str) -> Self {
            Self {
                url: format!("{url}/management"),
            }
        }
    }

    pub struct TestServer {
        pub root: PathBuf,
        pub url: String,
        pub account_id: Uuid,
        pub token: String,
        pub workspace_id: Uuid,
        task: JoinHandle<()>,
    }

    impl TestServer {
        pub async fn start() -> Self {
            let root =
                std::env::temp_dir().join(format!("block-dependencies-test-{}", Uuid::new_v4()));
            Self::start_at(root).await
        }

        pub async fn start_at(root: PathBuf) -> Self {
            let (url, task) = serve(&root).await;
            let management = Management::new(&url);
            let account = register(&management, &format!("{}@example.com", Uuid::new_v4())).await;
            let workspace = create_workspace(&management, &account.token, "Test").await;
            Self {
                root,
                url,
                account_id: account.id,
                token: account.token,
                workspace_id: workspace.id,
                task,
            }
        }

        pub async fn start_at_as(
            root: PathBuf,
            token: String,
            account_id: Uuid,
            workspace_id: Uuid,
        ) -> Self {
            let (url, task) = serve(&root).await;
            Self {
                root,
                url,
                account_id,
                token,
                workspace_id,
                task,
            }
        }

        pub fn management(&self) -> Management {
            Management::new(&self.url)
        }

        pub async fn connect(&self) -> Socket {
            self.connect_to(&self.token, self.workspace_id).await
        }

        pub async fn connect_to(&self, token: &str, workspace_id: Uuid) -> Socket {
            self.try_connect_to(token, workspace_id).await.unwrap()
        }

        /// Opens a block connection without asserting that the server accepted
        /// the handshake.
        pub async fn try_connect_to(
            &self,
            token: &str,
            workspace_id: Uuid,
        ) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
            let request = format!(
                "{}/?token={token}&workspace={workspace_id}",
                websocket_url(&self.url)
            )
            .into_client_request()
            .unwrap();
            connect_async(request).await.map(|(socket, _)| socket)
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

    async fn serve(root: &Path) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let root = root.to_path_buf();
        let task = tokio::spawn(async move {
            crate::serve(listener, root).await.unwrap();
        });
        (url, task)
    }

    fn websocket_url(url: &str) -> String {
        url.replacen("http://", "ws://", 1)
    }

    pub async fn request(socket: &mut Socket, message: ClientMessage) -> ServerMessage {
        socket
            .send(Message::Text(serde_json::to_string(&message).unwrap()))
            .await
            .unwrap();
        let message = socket.next().await.unwrap().unwrap();
        serde_json::from_str(&message.into_text().unwrap()).unwrap()
    }

    pub async fn management_request(
        management: &Management,
        message: ManagementClientMessage,
    ) -> ManagementServerMessage {
        let url = management.url.clone();
        let body = serde_json::to_vec(&message).unwrap();
        tokio::task::spawn_blocking(move || {
            let response = match ureq::post(&url)
                .set("content-type", "application/json")
                .send_bytes(&body)
            {
                Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                Err(error) => panic!("management request failed: {error}"),
            };
            serde_json::from_str(&response.into_string().unwrap()).unwrap()
        })
        .await
        .unwrap()
    }

    pub async fn register(management: &Management, email: &str) -> TestAccount {
        register_with_password(management, email, TEST_PASSWORD).await
    }

    pub async fn register_with_password(
        management: &Management,
        email: &str,
        password: &str,
    ) -> TestAccount {
        let response = management_request(
            management,
            ManagementClientMessage::Register {
                request_id: Uuid::new_v4(),
                email: email.into(),
                display_name: email.split('@').next().unwrap().into(),
                password: password.into(),
            },
        )
        .await;
        let ManagementServerMessage::Account { account, token, .. } = response else {
            panic!("registration failed: {response:?}");
        };
        TestAccount { account, token }
    }

    pub async fn create_workspace(management: &Management, token: &str, name: &str) -> Workspace {
        let response = management_request(
            management,
            ManagementClientMessage::CreateWorkspace {
                request_id: Uuid::new_v4(),
                token: token.into(),
                name: name.into(),
            },
        )
        .await;
        let ManagementServerMessage::Workspace { workspace, .. } = response else {
            panic!("workspace creation failed: {response:?}");
        };
        workspace
    }

    /// Invites `account` into the workspace with `role` and accepts on its
    /// behalf, leaving it a full member.
    pub async fn add_member(
        management: &Management,
        inviter_token: &str,
        workspace_id: Uuid,
        account: &TestAccount,
        role: WorkspaceRole,
    ) {
        let response = management_request(
            management,
            ManagementClientMessage::Invite {
                request_id: Uuid::new_v4(),
                token: inviter_token.into(),
                workspace_id,
                email: account.email.clone(),
                role,
            },
        )
        .await;
        let ManagementServerMessage::Invitation { invitation, .. } = response else {
            panic!("invitation failed: {response:?}");
        };
        let response = management_request(
            management,
            ManagementClientMessage::RespondInvitation {
                request_id: Uuid::new_v4(),
                token: account.token.clone(),
                invitation_id: invitation.id,
                accept: true,
            },
        )
        .await;
        assert!(
            matches!(response, ManagementServerMessage::Ok { .. }),
            "accepting the invitation failed: {response:?}"
        );
    }

    pub async fn list_access(socket: &mut Socket, id: Uuid) -> Vec<BlockAccessEntry> {
        match request(
            socket,
            ClientMessage::ListBlockAccess {
                request_id: Uuid::new_v4(),
                id,
            },
        )
        .await
        {
            ServerMessage::BlockAccessList { entries, .. } => entries,
            message => panic!("expected a block access list, got {message:?}"),
        }
    }

    pub async fn set_access(
        socket: &mut Socket,
        id: Uuid,
        account_id: Uuid,
        access: BlockAccess,
    ) -> ServerMessage {
        request(
            socket,
            ClientMessage::SetBlockAccess {
                request_id: Uuid::new_v4(),
                id,
                account_id,
                access,
            },
        )
        .await
    }

    pub fn access_for(entries: &[BlockAccessEntry], account_id: Uuid) -> BlockAccess {
        entries
            .iter()
            .find(|entry| entry.account.id == account_id)
            .unwrap_or_else(|| panic!("account {account_id} is not a workspace member"))
            .effective
    }

    pub async fn create(socket: &mut Socket, id: Uuid, references: Vec<Uuid>) -> ServerMessage {
        request(
            socket,
            ClientMessage::CreateBlock {
                request_id: Uuid::new_v4(),
                id,
                block_type: Uuid::new_v4(),
                data: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references,
                watch: false,
            },
        )
        .await
    }

    pub async fn update(
        socket: &mut Socket,
        id: Uuid,
        after: Vec<Uuid>,
        before: Vec<Uuid>,
    ) -> ServerMessage {
        request(
            socket,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id,
                seq: None,
                operation_id: Uuid::new_v4(),
                operation: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references: ReferenceDelta { before, after },
            },
        )
        .await
    }

    pub async fn set_parent(socket: &mut Socket, id: Uuid, parent: BlockParent) -> ServerMessage {
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

    pub async fn references(socket: &mut Socket, list: BlockReferenceList) -> Vec<BlockReference> {
        match request(
            socket,
            ClientMessage::ListReferences {
                request_id: Uuid::new_v4(),
                list,
                watch: false,
            },
        )
        .await
        {
            ServerMessage::References { blocks, .. } => blocks,
            message => panic!("expected references, got {message:?}"),
        }
    }

    pub fn parent(message: ServerMessage) -> BlockParent {
        match message {
            ServerMessage::ReadBlock { parent, .. } => parent,
            message => panic!("expected block read, got {message:?}"),
        }
    }

    pub async fn create_and_watch(socket: &mut Socket, id: Uuid) -> ServerMessage {
        request(
            socket,
            ClientMessage::CreateBlock {
                request_id: Uuid::new_v4(),
                id,
                block_type: Uuid::new_v4(),
                data: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references: vec![],
                watch: true,
            },
        )
        .await
    }

    pub async fn watch(socket: &mut Socket, id: Uuid) -> ServerMessage {
        request(
            socket,
            ClientMessage::ReadBlock {
                request_id: Uuid::new_v4(),
                id,
                watch: true,
            },
        )
        .await
    }

    pub async fn unwatch(socket: &mut Socket, id: Uuid) -> ServerMessage {
        request(
            socket,
            ClientMessage::UnwatchBlock {
                request_id: Uuid::new_v4(),
                id,
            },
        )
        .await
    }

    pub async fn post_presence(
        socket: &mut Socket,
        id: Uuid,
        presence_id: Uuid,
        data: Vec<u8>,
    ) -> ServerMessage {
        request(
            socket,
            ClientMessage::PostPresence {
                request_id: Uuid::new_v4(),
                id,
                presence_id,
                data,
            },
        )
        .await
    }

    pub async fn clear_presence(socket: &mut Socket, id: Uuid, presence_id: Uuid) -> ServerMessage {
        request(
            socket,
            ClientMessage::ClearPresence {
                request_id: Uuid::new_v4(),
                id,
                presence_id,
            },
        )
        .await
    }
}
