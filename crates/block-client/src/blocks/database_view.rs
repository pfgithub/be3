use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseViewSort {
    pub field_id: Uuid,
    pub direction: SortDirection,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseView {
    database_id: Uuid,
    sort: Option<DatabaseViewSort>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseViewOperation {
    SetDatabase {
        database_id: Uuid,
    },
    /// `None` clears the sort, leaving rows in their intrinsic (insertion) order.
    SetSort {
        sort: Option<DatabaseViewSort>,
    },
}

impl DatabaseView {
    pub fn new(database_id: Uuid) -> Self {
        Self {
            database_id,
            sort: None,
        }
    }

    pub fn database_id(&self) -> Uuid {
        self.database_id
    }

    pub fn sort(&self) -> Option<DatabaseViewSort> {
        self.sort
    }
}

impl Block for DatabaseView {
    type Operation = DatabaseViewOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x0000_6461_7461_6261_7365_2d76_6965_7701);
    const CRDT: bool = true;

    fn apply_operation(view: &mut Self, operation: &Self::Operation) {
        match operation {
            DatabaseViewOperation::SetDatabase { database_id } => {
                view.database_id = *database_id;
            }
            DatabaseViewOperation::SetSort { sort } => view.sort = *sort,
        }
    }

    fn references(&self) -> Vec<Uuid> {
        vec![self.database_id]
    }
}

#[cfg(test)]
mod tests;
