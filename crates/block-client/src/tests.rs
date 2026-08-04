use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};

use super::*;

mod a_read_guard_blocks_background_updates;
mod block_handle_history_is_shared_by_clones;
mod block_urls_include_workspace_and_reject_malformed_paths;
mod cached_blocks_are_populated_from_confirmed_metadata;
mod clear_dynamic_artifact_unlinks_the_block;
mod client_debug_snapshot_reports_active_worker_state;
mod created_blocks_are_immediately_readable_and_operate_optimistically;
mod duplicate_reference_watches_share_subscription;
mod dynamic_artifact_descriptor_survives_creation;
mod fetched_blocks_are_none_until_resolved;
mod finish_history_group_starts_a_new_action;
mod get_block_resolves_from_a_websocket_read_response;
mod history_metadata_counts_toward_eviction;
mod history_metadata_preserves_first_value_when_actions_merge;
mod history_metadata_round_trips_through_undo_and_redo;
mod management_client_reports_server_errors;
mod management_client_round_trips_account_and_workspace_operations;
mod matching_broadcast_before_acknowledgement_is_applied_once;
mod new_history_action_clears_redo;
mod no_history_policy_disables_undo;
mod remote_operations_rebuild_all_pending_optimistic_operations;
mod replace_preserves_dynamic_artifact_descriptor;
mod set_dynamic_artifact_updates_the_descriptor;
mod set_parent_optimistically_notes_backref;
mod wait_until_observes_current_and_future_values;
mod watched_operations_are_buffered_until_their_sequence_is_contiguous;
mod websocket_url_carries_the_connection_identity;
mod websocket_url_matches_the_server_scheme;

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
    type History = block::NoHistory;
    const TYPE_ID: Uuid = Uuid::from_u128(1);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let CounterOperation::Add(amount) = operation;
        block.count += amount;
    }

    fn implicit_name(&self) -> String {
        format!("Counter {}", self.count)
    }

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}
}

fn counter_snapshot(count: i64) -> Vec<u8> {
    serde_json::to_vec(&StoredBlock {
        value: Counter { count },
        dynamic_artifact: None,
    })
    .unwrap()
}

fn counter_operation(amount: i64) -> Vec<u8> {
    serde_json::to_vec(&StoredOperation::<Counter, CounterOperation>::Operate(
        CounterOperation::Add(amount),
    ))
    .unwrap()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct HistoryBlock {
    value: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum HistoryOperation {
    Set(i32),
}

struct HistoryPolicy;

struct HistoryAction {
    before: i32,
    after: i32,
}

impl Block for HistoryBlock {
    type Operation = HistoryOperation;
    type History = HistoryPolicy;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6869_7374_6f72_792d_7465_7374_0000_0001);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let HistoryOperation::Set(value) = operation;
        block.value = *value;
    }

    fn implicit_name(&self) -> String {
        "History test".into()
    }
}

impl BlockHistory<HistoryBlock> for HistoryPolicy {
    type Action = HistoryAction;
    type Snapshot = HistoryBlock;

    fn snapshot(block: &HistoryBlock) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: HistoryBlock,
        after: &HistoryBlock,
        _operations: &[HistoryOperation],
    ) -> Option<Self::Action> {
        (before != *after).then_some(HistoryAction {
            before: before.value,
            after: after.value,
        })
    }

    fn action_bytes(_action: &Self::Action) -> usize {
        size_of::<HistoryAction>()
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        previous.after = next.after;
        Ok(())
    }

    fn operations(
        _current: &HistoryBlock,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<HistoryOperation> {
        vec![HistoryOperation::Set(match direction {
            HistoryDirection::Undo => action.before,
            HistoryDirection::Redo => action.after,
        })]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DisabledHistoryBlock;

impl Block for DisabledHistoryBlock {
    type Operation = ();
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6869_7374_6f72_792d_7465_7374_0000_0002);

    fn apply_operation(_block: &mut Self, _operation: &Self::Operation) {}

    fn implicit_name(&self) -> String {
        "No history test".into()
    }
}

mod lib_test_support {
    pub(super) use super::{counter_operation, counter_snapshot, Counter, CounterOperation};
}

mod history_test_support {
    pub(super) use super::{DisabledHistoryBlock, HistoryBlock, HistoryOperation};
}
