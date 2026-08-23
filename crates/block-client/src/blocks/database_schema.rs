use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFieldType {
    String,
    Number,
    Enum,
}

                                                                          
                                                                         
                         
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseEnumOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseField {
    pub id: Uuid,
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub options: Vec<DatabaseEnumOption>,
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
    AddEnumOption {
        field_id: Uuid,
        option: DatabaseEnumOption,
    },
    RemoveEnumOption {
        field_id: Uuid,
        option_id: Uuid,
    },
    RenameEnumOption {
        field_id: Uuid,
        option_id: Uuid,
        name: String,
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
            DatabaseSchemaOperation::AddEnumOption { field_id, option } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    if !field
                        .options
                        .iter()
                        .any(|existing| existing.id == option.id)
                    {
                        field.options.push(option.clone());
                    }
                }
            }
            DatabaseSchemaOperation::RemoveEnumOption {
                field_id,
                option_id,
            } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.options.retain(|option| option.id != *option_id);
                }
            }
            DatabaseSchemaOperation::RenameEnumOption {
                field_id,
                option_id,
                name,
            } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    if let Some(option) = field
                        .options
                        .iter_mut()
                        .find(|option| option.id == *option_id)
                    {
                        option.name.clone_from(name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
