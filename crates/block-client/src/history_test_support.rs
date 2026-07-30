use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct HistoryBlock {
    pub(super) value: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) enum HistoryOperation {
    Set(i32),
}

pub(super) struct HistoryPolicy;

pub(super) struct HistoryAction {
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

    fn action(
        before: &HistoryBlock,
        after: &HistoryBlock,
        _operations: &[HistoryOperation],
    ) -> Option<Self::Action> {
        (before != after).then_some(HistoryAction {
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
pub(super) struct DisabledHistoryBlock;

impl Block for DisabledHistoryBlock {
    type Operation = ();
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6869_7374_6f72_792d_7465_7374_0000_0002);

    fn apply_operation(_block: &mut Self, _operation: &Self::Operation) {}

    fn implicit_name(&self) -> String {
        "No history test".into()
    }
}
