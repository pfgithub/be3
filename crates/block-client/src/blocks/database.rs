use std::collections::BTreeMap;

use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DatabaseValue {
    String(String),
    Number(f64),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatabaseRow {
    pub id: Uuid,
    values: BTreeMap<Uuid, DatabaseValue>,
}

impl DatabaseRow {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            values: BTreeMap::new(),
        }
    }

    pub fn values(&self) -> &BTreeMap<Uuid, DatabaseValue> {
        &self.values
    }

    pub fn value(&self, field_id: Uuid) -> Option<&DatabaseValue> {
        self.values.get(&field_id)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Database {
    schema_id: Uuid,
    rows: Vec<DatabaseRow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseOperation {
    SetSchema {
        schema_id: Uuid,
    },
    AddRow {
        row: DatabaseRow,
    },
    RemoveRow {
        row_id: Uuid,
    },
    SetCell {
        row_id: Uuid,
        field_id: Uuid,
        value: Option<DatabaseValue>,
    },
}

impl Database {
    pub fn new(schema_id: Uuid) -> Self {
        Self {
            schema_id,
            rows: Vec::new(),
        }
    }

    pub fn schema_id(&self) -> Uuid {
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
            DatabaseOperation::AddRow { row } => {
                if !database.rows.iter().any(|existing| existing.id == row.id) {
                    database.rows.push(row.clone());
                }
            }
            DatabaseOperation::RemoveRow { row_id } => {
                database.rows.retain(|row| row.id != *row_id);
            }
            DatabaseOperation::SetCell {
                row_id,
                field_id,
                value,
            } => {
                let Some(row) = database.rows.iter_mut().find(|row| row.id == *row_id) else {
                    return;
                };
                if let Some(value) = value {
                    row.values.insert(*field_id, value.clone());
                } else {
                    row.values.remove(field_id);
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        vec![self.schema_id]
    }
}

#[cfg(test)]
mod tests;
