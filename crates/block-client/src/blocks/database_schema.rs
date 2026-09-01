use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseFieldType {
    String,
    Number,
    Enum,
    Block,
    Boolean,
    Color,
    Datetime,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseNumberScale {
    #[default]
    Linear,
    Logarithmic,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DatabaseNumberOptions {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
    pub scale: DatabaseNumberScale,
}

impl DatabaseNumberOptions {
    pub fn effective_step(self) -> f64 {
        self.step.unwrap_or(match self.scale {
            DatabaseNumberScale::Linear => 1.0,
            DatabaseNumberScale::Logarithmic => 1.01,
        })
    }
    pub fn normalized(mut self) -> Self {
        self.minimum = self.minimum.filter(|value| value.is_finite());
        self.maximum = self.maximum.filter(|value| value.is_finite());
        self.step = self.step.filter(|value| value.is_finite());
        if self.scale == DatabaseNumberScale::Logarithmic {
            self.minimum = self.minimum.filter(|value| *value > 0.0);
            self.maximum = self.maximum.filter(|value| *value > 0.0);
        }
        if let (Some(minimum), Some(maximum)) = (self.minimum, self.maximum) {
            if minimum > maximum {
                self.minimum = Some(maximum);
                self.maximum = Some(minimum);
            }
        }
        self.step = match self.scale {
            DatabaseNumberScale::Linear => self.step.filter(|value| *value > 0.0),
            DatabaseNumberScale::Logarithmic => self.step.filter(|value| *value > 1.0),
        };
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseBlockOptions {
    pub block_type: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseEnumOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatabaseField {
    pub id: Uuid,
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub enum_options: Vec<DatabaseEnumOption>,
    pub number_options: DatabaseNumberOptions,
    pub block_options: DatabaseBlockOptions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DatabaseSchema {
    fields: Vec<DatabaseField>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    SetNumberOptions {
        field_id: Uuid,
        options: DatabaseNumberOptions,
    },
    SetBlockOptions {
        field_id: Uuid,
        options: DatabaseBlockOptions,
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
                    let mut field = field.clone();
                    field.number_options = field.number_options.normalized();
                    schema.fields.push(field);
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
            DatabaseSchemaOperation::SetNumberOptions { field_id, options } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.number_options = options.normalized();
                }
            }
            DatabaseSchemaOperation::SetBlockOptions { field_id, options } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.block_options = *options;
                }
            }
            DatabaseSchemaOperation::AddEnumOption { field_id, option } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    if !field
                        .enum_options
                        .iter()
                        .any(|existing| existing.id == option.id)
                    {
                        field.enum_options.push(option.clone());
                    }
                }
            }
            DatabaseSchemaOperation::RemoveEnumOption {
                field_id,
                option_id,
            } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    field.enum_options.retain(|option| option.id != *option_id);
                }
            }
            DatabaseSchemaOperation::RenameEnumOption {
                field_id,
                option_id,
                name,
            } => {
                if let Some(field) = schema.fields.iter_mut().find(|field| field.id == *field_id) {
                    if let Some(option) = field
                        .enum_options
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
