use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{StoredBlock, StoredOperation};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct Counter {
    pub(super) count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) enum CounterOperation {
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

pub(super) fn counter_snapshot(count: i64) -> Vec<u8> {
    serde_json::to_vec(&StoredBlock {
        value: Counter { count },
        dynamic_artifact: None,
    })
    .unwrap()
}

pub(super) fn counter_operation(amount: i64) -> Vec<u8> {
    serde_json::to_vec(&StoredOperation::<Counter, CounterOperation>::Operate(
        CounterOperation::Add(amount),
    ))
    .unwrap()
}
