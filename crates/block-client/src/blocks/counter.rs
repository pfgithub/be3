use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Counter {
    count: i64,
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self) -> i64 {
        self.count
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CounterOperation {
    Increment,
    Decrement,
}

impl Block for Counter {
    type Operation = CounterOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_0001);

    fn apply_operation(counter: &mut Self, operation: &Self::Operation) {
        counter.count = match operation {
            CounterOperation::Increment => counter.count.saturating_add(1),
            CounterOperation::Decrement => counter.count.saturating_sub(1),
        };
    }
}
