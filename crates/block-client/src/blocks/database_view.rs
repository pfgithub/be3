use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

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

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseViewKind {
    #[default]
    Spreadsheet,
    Kanban,
    Scatter,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseView {
    database_id: BlockRef,
    sort: Option<DatabaseViewSort>,
    kind: DatabaseViewKind,
                                                                       
    kanban_field_id: Option<Uuid>,
                                                              
    scatter_x_field_id: Option<Uuid>,
    scatter_y_field_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseViewOperation {
    SetDatabase {
        database_id: BlockRef,
    },
                                                                                  
    SetSort {
        sort: Option<DatabaseViewSort>,
    },
    SetKind {
        kind: DatabaseViewKind,
    },
    SetKanbanField {
        field_id: Option<Uuid>,
    },
    SetScatterXField {
        field_id: Option<Uuid>,
    },
    SetScatterYField {
        field_id: Option<Uuid>,
    },
}

impl DatabaseView {
    pub fn new(database_id: BlockRef) -> Self {
        Self {
            database_id,
            sort: None,
            kind: DatabaseViewKind::default(),
            kanban_field_id: None,
            scatter_x_field_id: None,
            scatter_y_field_id: None,
        }
    }

    pub fn database_id(&self) -> BlockRef {
        self.database_id
    }

    pub fn sort(&self) -> Option<DatabaseViewSort> {
        self.sort
    }

    pub fn kind(&self) -> DatabaseViewKind {
        self.kind
    }

    pub fn kanban_field_id(&self) -> Option<Uuid> {
        self.kanban_field_id
    }

    pub fn scatter_x_field_id(&self) -> Option<Uuid> {
        self.scatter_x_field_id
    }

    pub fn scatter_y_field_id(&self) -> Option<Uuid> {
        self.scatter_y_field_id
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
            DatabaseViewOperation::SetKind { kind } => view.kind = *kind,
            DatabaseViewOperation::SetKanbanField { field_id } => {
                view.kanban_field_id = *field_id;
            }
            DatabaseViewOperation::SetScatterXField { field_id } => {
                view.scatter_x_field_id = *field_id;
            }
            DatabaseViewOperation::SetScatterYField { field_id } => {
                view.scatter_y_field_id = *field_id;
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.database_id.as_direct().into_iter().collect()
    }
}

#[cfg(test)]
mod tests;
