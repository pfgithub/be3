use super::*;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

mod batch_is_acknowledged_before_watch_notifications;
mod batch_updates_apply_reference_deltas_in_request_order;
mod dependency_state_survives_a_server_restart;
mod explicit_name_overrides_implicit_names_and_updates_both_kinds_of_watch;
mod explicit_sequences_cannot_be_applied_out_of_order;
mod lists_backrefs_with_relationship_metadata;
mod lists_every_root_block_in_uuid_order;
mod lists_parents;
mod merges_reference_deltas_from_concurrent_clients;
mod missing_references_reject_creates_and_do_not_commit_updates;
mod names_longer_than_128_utf8_bytes_are_rejected;
mod omitted_sequences_are_assigned_by_the_server;
mod operation_ids_are_idempotent_and_conflicts_are_rejected;
mod parent_watch_updates;
mod preserves_reference_order;
mod read_returns_parent;
mod reads_replay_contiguous_operation_records;
mod reference_watch_updates_when_a_listed_blocks_parent_changes;
mod reference_watch_updates_when_a_listed_blocks_reference_count_changes;
mod rejects_missing_parent_references_and_parent_cycles;
mod removing_a_parent_reference_orphans_the_child_without_restoring_it_on_readd;
mod reparents_without_changing_either_parents_references;
mod sequence_errors_include_the_expected_sequence;
mod shared_protocol_round_trips_over_websocket;
mod watches_reference_changes_until_unwatched;

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
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
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

mod support {
    #![allow(dead_code)]

    use std::path::PathBuf;

    use block::{
        BlockParent, BlockReference, BlockReferenceList, ClientMessage, ReferenceDelta,
        ServerMessage,
    };
    use futures_util::{SinkExt, StreamExt};
    use tokio::{fs, net::TcpListener, task::JoinHandle};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
        MaybeTlsStream, WebSocketStream,
    };
    use uuid::Uuid;

    pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    pub struct TestServer {
        pub root: PathBuf,
        pub url: String,
        task: JoinHandle<()>,
    }

    impl TestServer {
        pub async fn start() -> Self {
            let root =
                std::env::temp_dir().join(format!("block-dependencies-test-{}", Uuid::new_v4()));
            Self::start_at(root).await
        }

        pub async fn start_at(root: PathBuf) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("ws://{}", listener.local_addr().unwrap());
            let server_root = root.clone();
            let task = tokio::spawn(async move {
                crate::serve(listener, server_root).await.unwrap();
            });
            Self { root, url, task }
        }

        pub async fn connect(&self) -> Socket {
            self.connect_as(Uuid::new_v4()).await
        }

        pub async fn connect_as(&self, account_id: Uuid) -> Socket {
            let mut request = self.url.as_str().into_client_request().unwrap();
            request.headers_mut().insert(
                "x-block-account-id",
                HeaderValue::from_str(&account_id.to_string()).unwrap(),
            );
            connect_async(request).await.unwrap().0
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
                implicit_name: "Block".into(),
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
                implicit_name: "Block".into(),
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
}
