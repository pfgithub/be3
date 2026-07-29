use std::{collections::HashMap, sync::Arc};

use block::{Block, BlockParent, BlockReference, BlockReferenceList, CommandKind, ServerMessage};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    BlockShared, CachedBlock, ClientDebugSnapshot, ErasedBlock, NetworkDebugSnapshot,
    PendingRequest, TypedBlock, WorkerState,
};

#[derive(Clone, Deserialize, Serialize)]
struct MetadataBlock;

impl Block for MetadataBlock {
    type Operation = ();

    const TYPE_ID: Uuid = Uuid::from_u128(0xfeed);

    fn apply_operation(_block: &mut Self, _operation: &Self::Operation) {}

    fn implicit_name(&self) -> String {
        "Created".into()
    }
}

fn state(cache: Arc<RwLock<HashMap<Uuid, CachedBlock>>>) -> WorkerState {
    WorkerState::new(
        Arc::new(RwLock::new(())),
        Arc::new(RwLock::new(NetworkDebugSnapshot::default())),
        Arc::new(RwLock::new(ClientDebugSnapshot::empty(Uuid::nil()))),
        cache,
    )
}

#[test]
fn cached_blocks_are_populated_from_confirmed_metadata() {
    let cache = Arc::new(RwLock::new(HashMap::new()));
    let mut state = state(Arc::clone(&cache));

    let created_id = Uuid::new_v4();
    let created: Arc<dyn ErasedBlock> = Arc::new(TypedBlock::created(
        created_id,
        Arc::new(BlockShared {
            value: RwLock::new(Some(MetadataBlock)),
        }),
        MetadataBlock,
    ));
    state.blocks.insert(created_id, created);
    let create_request = Uuid::new_v4();
    state
        .requests
        .insert(create_request, PendingRequest::Create { id: created_id });
    state.handle_server_message(ServerMessage::Ok {
        request_id: create_request,
        command: CommandKind::CreateBlock,
        id: created_id,
        seq: Some(0),
        operation_id: None,
    });
    assert_eq!(cache.read()[&created_id].name, "Created");

    let read_id = Uuid::new_v4();
    let read: Arc<dyn ErasedBlock> = Arc::new(TypedBlock::<MetadataBlock>::unresolved(
        read_id,
        Arc::new(BlockShared {
            value: RwLock::new(None),
        }),
    ));
    state.blocks.insert(read_id, read);
    let read_request = Uuid::new_v4();
    state
        .requests
        .insert(read_request, PendingRequest::Read { id: read_id });
    state.handle_server_message(ServerMessage::ReadBlock {
        request_id: read_request,
        command: CommandKind::ReadBlock,
        id: read_id,
        block_type: MetadataBlock::TYPE_ID,
        snapshot: serde_json::to_vec(&MetadataBlock).unwrap(),
        snapshot_seq: 0,
        operations: Vec::new(),
        parent: BlockParent::Root,
        name: "Read".into(),
    });
    assert_eq!(cache.read()[&read_id].name, "Read");

    let listed_id = Uuid::new_v4();
    let list = BlockReferenceList::Roots;
    let list_request = Uuid::new_v4();
    state
        .requests
        .insert(list_request, PendingRequest::CacheReferences { list });
    state.handle_server_message(ServerMessage::References {
        request_id: list_request,
        list,
        blocks: vec![BlockReference {
            id: listed_id,
            block_type: Uuid::from_u128(0xbeef),
            name: "Listed".into(),
            parent: BlockParent::Root,
            references: 0,
        }],
    });
    assert_eq!(cache.read()[&listed_id].name, "Listed");

    state.handle_server_message(ServerMessage::BlockNameUpdated {
        id: listed_id,
        name: "Renamed".into(),
    });
    assert_eq!(cache.read()[&listed_id].name, "Renamed");
}
