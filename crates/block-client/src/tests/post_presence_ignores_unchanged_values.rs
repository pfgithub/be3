use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    ClientDebugSnapshot, DeferredRequest, NetworkDebugSnapshot, WorkerCommand, WorkerState,
};

fn state() -> WorkerState {
    WorkerState::new(
        Arc::new(RwLock::new(())),
        Arc::new(RwLock::new(NetworkDebugSnapshot::default())),
        Arc::new(RwLock::new(ClientDebugSnapshot::empty(
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
        ))),
        Arc::new(RwLock::new(HashMap::new())),
        Arc::new(RwLock::new(HashMap::new())),
        Arc::new(RwLock::new(HashMap::new())),
    )
}

#[test]
fn post_presence_ignores_unchanged_values() {
    let mut state = state();
    let id = Uuid::from_u128(1);
    let presence_id = Uuid::from_u128(2);

    state.handle_command(WorkerCommand::PostPresence {
        id,
        presence_id,
        data: vec![1],
    });
    state.handle_command(WorkerCommand::PostPresence {
        id,
        presence_id,
        data: vec![1],
    });
    state.handle_command(WorkerCommand::PostPresence {
        id,
        presence_id,
        data: vec![2],
    });
    state.handle_command(WorkerCommand::ClearPresence { id, presence_id });
    state.handle_command(WorkerCommand::PostPresence {
        id,
        presence_id,
        data: vec![2],
    });

    assert!(matches!(
        state.deferred.make_contiguous(),
        [
            DeferredRequest::PostPresence { data, .. },
            DeferredRequest::PostPresence { data: changed_data, .. },
            DeferredRequest::ClearPresence { .. },
            DeferredRequest::PostPresence { data: reposted_data, .. },
        ] if *data == [1] && *changed_data == [2] && *reposted_data == [2]
    ));
}
