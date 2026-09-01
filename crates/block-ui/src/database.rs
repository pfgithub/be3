use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
    time::{SystemTime, UNIX_EPOCH},
};

use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::{DatabaseColor, DatabaseRow, DatabaseValue},
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseNumberOptions, DatabaseNumberScale,
        },
    },
};
use uuid::Uuid;

use crate::{
    datetime::{datetime_editor, format_datetime_utc, parse_datetime_utc},
    test_id::TestId,
    BlockLabel,
};

#[derive(Debug, PartialEq)]
pub struct DatabaseValueChange {
    pub field_id: Uuid,
    pub value: Option<DatabaseValue>,
    pub continuous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseBlockPickRequest {
    pub field_id: Uuid,
    pub block_type: Option<Uuid>,
}

#[derive(Debug, Default, PartialEq)]
pub struct DatabaseValueEditorOutput {
    pub changes: Vec<DatabaseValueChange>,
    pub block_pick: Option<DatabaseBlockPickRequest>,
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
        block_labels: &HashMap<BlockRef, BlockLabel>,
        test_id_prefix: &str,
    ) -> DatabaseValueEditorOutput {
        self.buffers
            .retain(|field_id, _| fields.iter().any(|field| field.id == *field_id));
        let mut output = DatabaseValueEditorOutput::default();
        for field in fields {
            ui.label(format!(
                "{} ({})",
                field.name,
                field_type_label(field.field_type)
            ));
            let state = value_state(values, field.id);
            match field.field_type {
                DatabaseFieldType::String => {
                    let buffer = self.buffers.entry(field.id).or_insert_with(|| {
                        let (text, mixed) = match state {
                            ValueState::Uniform(Some(value)) => {
                                (database_value_text(value, field, block_labels), false)
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
                        output.changes.push(DatabaseValueChange {
                            field_id: field.id,
                            value: Some(DatabaseValue::String(buffer.text.clone())),
                            continuous: true,
                        });
                    }
                }
                DatabaseFieldType::Number => {
                    let mut value = match state {
                        ValueState::Uniform(Some(DatabaseValue::Number(value))) => *value,
                        _ => initial_number_value(field.number_options),
                    };
                    if number_drag_value(ui, &mut value, field.number_options).changed() {
                        output.changes.push(DatabaseValueChange {
                            field_id: field.id,
                            value: Some(DatabaseValue::Number(value)),
                            continuous: true,
                        });
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
                                .and_then(|id| {
                                    field.enum_options.iter().find(|option| option.id == id)
                                })
                                .map_or("None", |option| option.name.as_str());
                            (current, name)
                        }
                    };
                    egui::ComboBox::new((test_id_prefix, field.id), "")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for option in &field.enum_options {
                                if ui
                                    .selectable_label(current == Some(option.id), &option.name)
                                    .clicked()
                                {
                                    output.changes.push(DatabaseValueChange {
                                        field_id: field.id,
                                        value: Some(DatabaseValue::Enum(option.id)),
                                        continuous: false,
                                    });
                                }
                            }
                        });
                }
                DatabaseFieldType::Boolean => {
                    let mut value = matches!(
                        state,
                        ValueState::Uniform(Some(DatabaseValue::Boolean(true)))
                    );
                    let text = if state == ValueState::Mixed {
                        "Mixed"
                    } else {
                        ""
                    };
                    if ui.checkbox(&mut value, text).changed() {
                        output.changes.push(DatabaseValueChange {
                            field_id: field.id,
                            value: Some(DatabaseValue::Boolean(value)),
                            continuous: false,
                        });
                    }
                }
                DatabaseFieldType::Color => {
                    let mut color = match state {
                        ValueState::Uniform(Some(DatabaseValue::Color(color))) => {
                            [color.red, color.green, color.blue, color.alpha]
                        }
                        _ => [255, 255, 255, 255],
                    };
                    if ui
                        .color_edit_button_srgba_unmultiplied(&mut color)
                        .changed()
                    {
                        output.changes.push(DatabaseValueChange {
                            field_id: field.id,
                            value: Some(DatabaseValue::Color(DatabaseColor {
                                red: color[0],
                                green: color[1],
                                blue: color[2],
                                alpha: color[3],
                            })),
                            continuous: true,
                        });
                    }
                }
                DatabaseFieldType::Datetime => match state {
                    ValueState::Uniform(Some(DatabaseValue::Datetime(current))) => {
                        let mut value = *current;
                        if datetime_editor(ui, &mut value) {
                            output.changes.push(DatabaseValueChange {
                                field_id: field.id,
                                value: Some(DatabaseValue::Datetime(value)),
                                continuous: true,
                            });
                        }
                    }
                    _ => {
                        if ui.button("Set").clicked() {
                            output.changes.push(DatabaseValueChange {
                                field_id: field.id,
                                value: Some(DatabaseValue::Datetime(current_utc_minute())),
                                continuous: true,
                            });
                        }
                    }
                },
                DatabaseFieldType::Block => {
                    let current = match state {
                        ValueState::Uniform(Some(DatabaseValue::Block(reference))) => {
                            Some(reference)
                        }
                        _ => None,
                    };
                    let label = current.map_or("Choose block".to_owned(), |reference| {
                        format!("Change: {}", block_reference_text(reference, block_labels))
                    });
                    if ui.button(label).clicked() {
                        output.block_pick = Some(DatabaseBlockPickRequest {
                            field_id: field.id,
                            block_type: field.block_options.block_type,
                        });
                    }
                }
            }
            if state != ValueState::Uniform(None) && ui.small_button("Clear").clicked() {
                output.changes.push(DatabaseValueChange {
                    field_id: field.id,
                    value: None,
                    continuous: false,
                });
            }
            ui.add_space(8.0);
        }
        output
    }
}

pub fn number_drag_value(
    ui: &mut egui::Ui,
    value: &mut f64,
    options: DatabaseNumberOptions,
) -> egui::Response {
    match options.scale {
        DatabaseNumberScale::Linear => ui.add(
            egui::DragValue::new(value)
                .speed(options.effective_step())
                .range(number_range(options))
                .clamp_existing_to_range(false),
        ),
        DatabaseNumberScale::Logarithmic => {
            let factor = options.effective_step();
            let base = factor.ln();
            let range = log_number_range(options, base);
            ui.add(
                egui::DragValue::from_get_set(|new_exponent| {
                    if let Some(new_exponent) = new_exponent {
                        *value = factor.powf(new_exponent);
                    }
                    value.max(f64::MIN_POSITIVE).ln() / base
                })
                .speed(1.0)
                .range(range)
                .clamp_existing_to_range(false)
                .custom_formatter(move |exponent, _| factor.powf(exponent).to_string())
                .custom_parser(move |text| {
                    text.parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .map(|value| value.ln() / base)
                }),
            )
        }
    }
}

fn number_range(options: DatabaseNumberOptions) -> RangeInclusive<f64> {
    options.minimum.unwrap_or(f64::NEG_INFINITY)..=options.maximum.unwrap_or(f64::INFINITY)
}

fn log_number_range(options: DatabaseNumberOptions, base: f64) -> RangeInclusive<f64> {
    options
        .minimum
        .map_or(f64::NEG_INFINITY, |value| value.ln() / base)
        ..=options
            .maximum
            .map_or(f64::INFINITY, |value| value.ln() / base)
}

fn initial_number_value(options: DatabaseNumberOptions) -> f64 {
    match options.scale {
        DatabaseNumberScale::Linear => 0.0,
        DatabaseNumberScale::Logarithmic => options.minimum.unwrap_or(1.0),
    }
}

fn current_utc_minute() -> i64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64);
    seconds - seconds.rem_euclid(60)
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

pub fn cell_text(
    row: &DatabaseRow,
    field: &DatabaseField,
    block_labels: &HashMap<BlockRef, BlockLabel>,
) -> String {
    match row.value(field.id) {
        Some(value) => database_value_text(value, field, block_labels),
        None => String::new(),
    }
}

pub fn database_value_text(
    value: &DatabaseValue,
    field: &DatabaseField,
    block_labels: &HashMap<BlockRef, BlockLabel>,
) -> String {
    match value {
        DatabaseValue::String(value) => value.clone(),
        DatabaseValue::Number(value) => value.to_string(),
        DatabaseValue::Enum(id) => field
            .enum_options
            .iter()
            .find(|option| option.id == *id)
            .map_or_else(String::new, |option| option.name.clone()),
        DatabaseValue::Block(reference) => block_reference_text(reference, block_labels),
        DatabaseValue::Boolean(value) => value.to_string(),
        DatabaseValue::Color(color) => format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.red, color.green, color.blue, color.alpha
        ),
        DatabaseValue::Datetime(value) => format_datetime_utc(*value),
    }
}

pub fn block_reference_text(
    reference: &BlockRef,
    block_labels: &HashMap<BlockRef, BlockLabel>,
) -> String {
    block_labels.get(reference).map_or_else(
        || match reference {
            BlockRef::Direct(id) => id.to_string(),
            BlockRef::RepoRelative { eternal_id, .. } => eternal_id.to_string(),
        },
        |label| label.name.clone(),
    )
}

pub fn parse_cell_value(value: &str, field: &DatabaseField) -> Option<DatabaseValue> {
    match field.field_type {
        DatabaseFieldType::String => Some(DatabaseValue::String(value.to_owned())),
        DatabaseFieldType::Number => {
            let mut value = value.parse::<f64>().ok()?;
            if !value.is_finite()
                || (field.number_options.scale == DatabaseNumberScale::Logarithmic && value <= 0.0)
            {
                return None;
            }
            if let Some(minimum) = field.number_options.minimum {
                value = value.max(minimum);
            }
            if let Some(maximum) = field.number_options.maximum {
                value = value.min(maximum);
            }
            Some(DatabaseValue::Number(value))
        }
        DatabaseFieldType::Enum => field
            .enum_options
            .iter()
            .find(|option| option.name == value)
            .map(|option| DatabaseValue::Enum(option.id)),
        DatabaseFieldType::Block => None,
        DatabaseFieldType::Boolean => value.parse().ok().map(DatabaseValue::Boolean),
        DatabaseFieldType::Color => {
            let value = value.strip_prefix('#')?;
            if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).ok();
            Some(DatabaseValue::Color(DatabaseColor {
                red: channel(0)?,
                green: channel(2)?,
                blue: channel(4)?,
                alpha: channel(6)?,
            }))
        }
        DatabaseFieldType::Datetime => parse_datetime_utc(value).map(DatabaseValue::Datetime),
    }
}

pub fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "Text",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Enum",
        DatabaseFieldType::Block => "Block",
        DatabaseFieldType::Boolean => "Boolean",
        DatabaseFieldType::Color => "Color",
        DatabaseFieldType::Datetime => "Datetime",
    }
}

#[cfg(test)]
mod tests;
