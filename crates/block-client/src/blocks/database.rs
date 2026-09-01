use std::collections::{BTreeMap, HashSet};

use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DatabaseValue {
    String(String),
    Number(f64),
    Enum(Uuid),
    Block(BlockRef),
    Boolean(bool),
    Color(DatabaseColor),
    Datetime(i64),
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DatabaseRow {
    values: BTreeMap<Uuid, DatabaseValue>,
}

impl DatabaseRow {
    pub fn values(&self) -> &BTreeMap<Uuid, DatabaseValue> {
        &self.values
    }

    pub fn value(&self, field_id: Uuid) -> Option<&DatabaseValue> {
        self.values.get(&field_id)
    }

    fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Database {
    schema_id: BlockRef,
    rows: Vec<DatabaseRow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseOperation {
    SetSchema {
        schema_id: BlockRef,
    },
    SetCell {
        row_index: usize,
        field_id: Uuid,
        value: Option<DatabaseValue>,
    },
}

impl Database {
    pub fn new(schema_id: BlockRef) -> Self {
        Self {
            schema_id,
            rows: Vec::new(),
        }
    }

    pub fn schema_id(&self) -> BlockRef {
        self.schema_id
    }

    pub fn rows(&self) -> &[DatabaseRow] {
        &self.rows
    }
}

impl Block for Database {
    type Operation = DatabaseOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x0064_6174_6162_6173_652d_626c_6f63_6b01);
    const CRDT: bool = true;

    fn apply_operation(database: &mut Self, operation: &Self::Operation) {
        match operation {
            DatabaseOperation::SetSchema { schema_id } => database.schema_id = *schema_id,
            DatabaseOperation::SetCell {
                row_index,
                field_id,
                value,
            } => {
                if *row_index >= database.rows.len() {
                    database.rows.resize(row_index + 1, DatabaseRow::default());
                }
                let row = &mut database.rows[*row_index];
                if let Some(value) = value {
                    row.values.insert(*field_id, value.clone());
                } else {
                    row.values.remove(field_id);
                }
                while database.rows.last().is_some_and(DatabaseRow::is_empty) {
                    database.rows.pop();
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        std::iter::once(self.schema_id)
            .chain(
                self.rows
                    .iter()
                    .flat_map(|row| row.values.values())
                    .filter_map(|value| match value {
                        DatabaseValue::Block(reference) => Some(*reference),
                        _ => None,
                    }),
            )
            .filter_map(|reference| reference.as_direct())
            .filter(|reference| seen.insert(*reference))
            .collect()
    }

    fn delete_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        let reference = BlockRef::Direct(block_id);
        Some(
            self.rows
                .iter()
                .enumerate()
                .flat_map(|(row_index, row)| {
                    row.values.iter().filter_map(move |(field_id, value)| {
                        (value == &DatabaseValue::Block(reference)).then_some(
                            DatabaseOperation::SetCell {
                                row_index,
                                field_id: *field_id,
                                value: None,
                            },
                        )
                    })
                })
                .collect(),
        )
    }

    fn replace_child(&self, old: Uuid, new: Uuid) -> Option<Vec<Self::Operation>> {
        let old = BlockRef::Direct(old);
        let new = BlockRef::Direct(new);
        Some(
            self.rows
                .iter()
                .enumerate()
                .flat_map(|(row_index, row)| {
                    row.values.iter().filter_map(move |(field_id, value)| {
                        (value == &DatabaseValue::Block(old)).then_some(
                            DatabaseOperation::SetCell {
                                row_index,
                                field_id: *field_id,
                                value: Some(DatabaseValue::Block(new)),
                            },
                        )
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests;
