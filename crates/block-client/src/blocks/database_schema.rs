use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFieldType {
    String,
    Number,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseField {
    pub id: Uuid,
    pub name: String,
    pub field_type: DatabaseFieldType,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseSchema {
    fields: Vec<DatabaseField>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DatabaseSchemaOperation {
    AddField {
        field: DatabaseField,
    },
    RemoveField {
        field_id: Uuid,
    },
    RenameField {
        field_id: Uuid,
        name: String,
    },
    SetFieldType {
        field_id: Uuid,
        field_type: DatabaseFieldType,
    },
}

impl DatabaseSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fields(&self) -> &[DatabaseField] {
        &self.fields
    }
}

impl Block for DatabaseSchema {
    type Operation = DatabaseSchemaOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6461_7461_6261_7365_2d73_6368_656d_6101);
    const CRDT: bool = true;

    fn apply_operation(schema: &mut Self, operation: &Self::Operation) {
        match operation {
            DatabaseSchemaOperation::AddField { field } => {
                if !schema.fields.iter().any(|existing| existing.id == field.id) {
                    schema.fields.push(field.clone());
                }
            }
            DatabaseSchemaOperation::RemoveField { field_id } => {
                schema.fields.retain(|field| field.id != *field_id);
            }
            DatabaseSchemaOperation::RenameField { field_id, name } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.name.clone_from(name);
                }
            }
            DatabaseSchemaOperation::SetFieldType {
                field_id,
                field_type,
            } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.field_type = *field_type;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
