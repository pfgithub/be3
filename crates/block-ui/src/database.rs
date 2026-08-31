use std::collections::BTreeMap;

use block_client::blocks::database::{DatabaseRow, DatabaseValue};
use block_client::blocks::database_schema::{DatabaseField, DatabaseFieldType};
use uuid::Uuid;

use crate::test_id::TestId;

#[derive(Debug, PartialEq)]
pub struct DatabaseValueChange {
    pub field_id: Uuid,
    pub value: Option<DatabaseValue>,
    pub continuous: bool,
}

#[derive(Default)]
pub struct DatabaseValueEditor {
    buffers: BTreeMap<Uuid, ValueBuffer>,
}

struct ValueBuffer {
    text: String,
    mixed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ValueState<'a> {
    Uniform(Option<&'a DatabaseValue>),
    Mixed,
}

impl DatabaseValueEditor {
    pub fn reset(&mut self) {
        self.buffers.clear();
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        fields: &[DatabaseField],
        values: &[&BTreeMap<Uuid, DatabaseValue>],
        test_id_prefix: &str,
    ) -> Vec<DatabaseValueChange> {
        self.buffers
            .retain(|field_id, _| fields.iter().any(|field| field.id == *field_id));
        let mut changes = Vec::new();
        for field in fields {
            ui.label(format!(
                "{} ({})",
                field.name,
                field_type_label(field.field_type)
            ));
            let state = value_state(values, field.id);
            match field.field_type {
                DatabaseFieldType::String | DatabaseFieldType::Number => {
                    let buffer = self.buffers.entry(field.id).or_insert_with(|| {
                        let (text, mixed) = match state {
                            ValueState::Uniform(Some(value)) => {
                                (database_value_text(value, field), false)
                            }
                            ValueState::Uniform(None) => (String::new(), false),
                            ValueState::Mixed => (String::new(), true),
                        };
                        ValueBuffer { text, mixed }
                    });
                    let mut edit =
                        egui::TextEdit::singleline(&mut buffer.text).desired_width(f32::INFINITY);
                    if buffer.mixed {
                        edit = edit.hint_text("Mixed");
                    }
                    let response = ui
                        .add(edit)
                        .test_id(&format!("{test_id_prefix}.field.{}", field.id));
                    if response.changed() {
                        buffer.mixed = false;
                        let value = if field.field_type == DatabaseFieldType::Number
                            && buffer.text.trim().is_empty()
                        {
                            Some(None)
                        } else {
                            parse_cell_value(&buffer.text, field).map(Some)
                        };
                        if let Some(value) = value {
                            changes.push(DatabaseValueChange {
                                field_id: field.id,
                                value,
                                continuous: true,
                            });
                        }
                    }
                }
                DatabaseFieldType::Enum => {
                    let (current, selected_text) = match state {
                        ValueState::Mixed => (None, "Mixed"),
                        ValueState::Uniform(value) => {
                            let current = value.and_then(|value| match value {
                                DatabaseValue::Enum(id) => Some(*id),
                                _ => None,
                            });
                            let name = current
                                .and_then(|id| field.options.iter().find(|option| option.id == id))
                                .map_or("None", |option| option.name.as_str());
                            (current, name)
                        }
                    };
                    egui::ComboBox::new((test_id_prefix, field.id), "")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current.is_none(), "None").clicked() {
                                changes.push(DatabaseValueChange {
                                    field_id: field.id,
                                    value: None,
                                    continuous: false,
                                });
                            }
                            for option in &field.options {
                                if ui
                                    .selectable_label(current == Some(option.id), &option.name)
                                    .clicked()
                                {
                                    changes.push(DatabaseValueChange {
                                        field_id: field.id,
                                        value: Some(DatabaseValue::Enum(option.id)),
                                        continuous: false,
                                    });
                                }
                            }
                        });
                }
            }
            ui.add_space(8.0);
        }
        changes
    }
}

fn value_state<'a>(values: &[&'a BTreeMap<Uuid, DatabaseValue>], field_id: Uuid) -> ValueState<'a> {
    let first = values.first().and_then(|values| values.get(&field_id));
    if values
        .iter()
        .skip(1)
        .all(|values| values.get(&field_id) == first)
    {
        ValueState::Uniform(first)
    } else {
        ValueState::Mixed
    }
}

pub fn cell_text(row: &DatabaseRow, field: &DatabaseField) -> String {
    match row.value(field.id) {
        Some(value) => database_value_text(value, field),
        None => String::new(),
    }
}

pub fn database_value_text(value: &DatabaseValue, field: &DatabaseField) -> String {
    match value {
        DatabaseValue::String(value) => value.clone(),
        DatabaseValue::Number(value) => value.to_string(),
        DatabaseValue::Enum(id) => field
            .options
            .iter()
            .find(|option| option.id == *id)
            .map_or_else(String::new, |option| option.name.clone()),
    }
}

pub fn parse_cell_value(value: &str, field: &DatabaseField) -> Option<DatabaseValue> {
    match field.field_type {
        DatabaseFieldType::String => Some(DatabaseValue::String(value.to_owned())),
        DatabaseFieldType::Number => value.parse().ok().map(DatabaseValue::Number),
        DatabaseFieldType::Enum => field
            .options
            .iter()
            .find(|option| option.name == value)
            .map(|option| DatabaseValue::Enum(option.id)),
    }
}

pub fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "Text",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Enum",
    }
}

#[cfg(test)]
mod tests;
