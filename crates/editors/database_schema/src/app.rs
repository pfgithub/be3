use std::sync::Arc;

use block_client::blocks::database_schema::{
    DatabaseBlockOptions, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseNumberScale, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{
    block_ui::{test_id::TestId, BlockCatalog},
    egui,
    egui_material_icons::icons::{ICON_ADD, ICON_DELETE},
    EditorHost,
};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 600.0;
const ROW_HEIGHT: f32 = 40.0;
const SECTION_SPACING: f32 = 10.0;
#[derive(Default)]
pub struct DatabaseSchemaApp {
    host: Option<EditorHost>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<DatabaseSchema>>,
}

impl block_editor_plugin::App for DatabaseSchemaApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(DatabaseSchema::new()).id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let schema = self.block.as_ref()?.read()?;
        let lines = schema.fields().iter().map(field_line_count).sum::<usize>() + 2;
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            ROW_HEIGHT * lines as f32 + SECTION_SPACING * schema.fields().len() as f32,
        ))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(block) = self.block.clone() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let editable = self.host.as_ref().is_none_or(EditorHost::editable);
        ui.heading("Fields");
        ui.add_space(8.0);
        let catalog = self.host.as_ref().map(EditorHost::block_types);
        fields_ui(ui, &block, editable, catalog.as_deref());
    }
}

pub fn fields_ui(
    ui: &mut egui::Ui,
    block: &BlockHandle<DatabaseSchema>,
    editable: bool,
    catalog: Option<&BlockCatalog>,
) {
    let Some(schema) = block.read() else {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return;
    };
    let fields = schema.fields().to_vec();
    drop(schema);

    let mut operations = Vec::new();
    ui.add_enabled_ui(editable, |ui| {
        ui.vertical(|ui| {
            for field in fields {
                ui.horizontal(|ui| {
                    let mut name = field.name.clone();
                    if ui
                        .add(egui::TextEdit::singleline(&mut name).desired_width(240.0))
                        .test_id(&format!("database-schema.name.{}", field.id))
                        .changed()
                    {
                        operations.push(DatabaseSchemaOperation::RenameField {
                            field_id: field.id,
                            name,
                        });
                    }

                    let mut field_type = field.field_type;
                    let combo = egui::ComboBox::from_id_salt(("field-type", field.id))
                        .selected_text(field_type_label(field_type))
                        .show_ui(ui, |ui| {
                            for (value, label, id) in [
                                (DatabaseFieldType::String, "String", "string"),
                                (DatabaseFieldType::Number, "Number", "number"),
                                (DatabaseFieldType::Enum, "Enum", "enum"),
                                (DatabaseFieldType::Block, "Block", "block"),
                                (DatabaseFieldType::Boolean, "Boolean", "boolean"),
                                (DatabaseFieldType::Color, "Color", "color"),
                                (DatabaseFieldType::Datetime, "Datetime", "datetime"),
                            ] {
                                ui.selectable_value(&mut field_type, value, label)
                                    .test_id(&format!("database-schema.type.{}.{}", field.id, id));
                            }
                        });
                    combo
                        .response
                        .test_id(&format!("database-schema.type.{}", field.id));
                    if field_type != field.field_type {
                        operations.push(DatabaseSchemaOperation::SetFieldType {
                            field_id: field.id,
                            field_type,
                        });
                    }

                    if ui
                        .small_button(ICON_DELETE)
                        .on_hover_text("Delete field")
                        .test_id(&format!("database-schema.delete.{}", field.id))
                        .clicked()
                    {
                        operations
                            .push(DatabaseSchemaOperation::RemoveField { field_id: field.id });
                    }
                });

                ui.indent(("database-schema-settings", field.id), |ui| {
                    match field.field_type {
                        DatabaseFieldType::Number => {
                            number_options_ui(ui, &field, &mut operations);
                        }
                        DatabaseFieldType::Enum => {
                            enum_options_ui(ui, &field, &mut operations);
                        }
                        DatabaseFieldType::Block => {
                            block_options_ui(ui, &field, catalog, &mut operations);
                        }
                        DatabaseFieldType::String
                        | DatabaseFieldType::Boolean
                        | DatabaseFieldType::Color
                        | DatabaseFieldType::Datetime => {}
                    }
                });
                ui.add_space(SECTION_SPACING);
                ui.separator();
            }
        });

        ui.add_space(8.0);
        if ui
            .button(format!("{} Add field", ICON_ADD.codepoint))
            .test_id("database-schema.add-field")
            .clicked()
        {
            operations.push(DatabaseSchemaOperation::AddField {
                field: DatabaseField {
                    id: Uuid::new_v4(),
                    name: "Field".into(),
                    field_type: DatabaseFieldType::String,
                    enum_options: Vec::new(),
                    number_options: Default::default(),
                    block_options: Default::default(),
                },
            });
        }
    });
    for operation in operations {
        block.operate(operation);
    }
}

fn number_options_ui(
    ui: &mut egui::Ui,
    field: &DatabaseField,
    operations: &mut Vec<DatabaseSchemaOperation>,
) {
    let mut options = field.number_options;
    let mut changed = false;
    changed |= optional_number_ui(
        ui,
        "Minimum",
        &mut options.minimum,
        &format!("database-schema.number.{}.minimum", field.id),
        0.0,
    );
    changed |= optional_number_ui(
        ui,
        "Maximum",
        &mut options.maximum,
        &format!("database-schema.number.{}.maximum", field.id),
        100.0,
    );
    let effective_step = options.effective_step();
    changed |= optional_number_ui(
        ui,
        "Step",
        &mut options.step,
        &format!("database-schema.number.{}.step", field.id),
        effective_step,
    );
    ui.horizontal(|ui| {
        ui.label("Scale");
        let combo = egui::ComboBox::from_id_salt(("number-scale", field.id))
            .selected_text(number_scale_label(options.scale))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut options.scale, DatabaseNumberScale::Linear, "Linear")
                    .changed();
                changed |= ui
                    .selectable_value(
                        &mut options.scale,
                        DatabaseNumberScale::Logarithmic,
                        "Logarithmic",
                    )
                    .changed();
            });
        combo
            .response
            .test_id(&format!("database-schema.number.{}.scale", field.id));
    });
    if changed {
        operations.push(DatabaseSchemaOperation::SetNumberOptions {
            field_id: field.id,
            options,
        });
    }
}

fn optional_number_ui(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    test_id: &str,
    default: f64,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *value = enabled.then_some(default);
            changed = true;
        }
        let mut current = value.unwrap_or(default);
        let response = ui
            .add_enabled(enabled, egui::DragValue::new(&mut current))
            .test_id(test_id);
        if response.changed() {
            *value = Some(current);
            changed = true;
        }
    });
    changed
}

fn enum_options_ui(
    ui: &mut egui::Ui,
    field: &DatabaseField,
    operations: &mut Vec<DatabaseSchemaOperation>,
) {
    for option in &field.enum_options {
        ui.horizontal(|ui| {
            let mut name = option.name.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut name).desired_width(240.0))
                .test_id(&format!(
                    "database-schema.enum.{}.{}.name",
                    field.id, option.id
                ))
                .changed()
            {
                operations.push(DatabaseSchemaOperation::RenameEnumOption {
                    field_id: field.id,
                    option_id: option.id,
                    name,
                });
            }
            if ui
                .small_button(ICON_DELETE)
                .on_hover_text("Delete option")
                .test_id(&format!(
                    "database-schema.enum.{}.{}.delete",
                    field.id, option.id
                ))
                .clicked()
            {
                operations.push(DatabaseSchemaOperation::RemoveEnumOption {
                    field_id: field.id,
                    option_id: option.id,
                });
            }
        });
    }
    ui.horizontal(|ui| {
        if ui
            .small_button(ICON_ADD)
            .on_hover_text("Add option")
            .test_id(&format!("database-schema.enum.{}.add", field.id))
            .clicked()
        {
            operations.push(DatabaseSchemaOperation::AddEnumOption {
                field_id: field.id,
                option: DatabaseEnumOption {
                    id: Uuid::new_v4(),
                    name: "Option".into(),
                },
            });
        }
        ui.label("Add option");
    });
}

fn block_options_ui(
    ui: &mut egui::Ui,
    field: &DatabaseField,
    catalog: Option<&BlockCatalog>,
    operations: &mut Vec<DatabaseSchemaOperation>,
) {
    let mut entries = catalog
        .into_iter()
        .flat_map(BlockCatalog::iter)
        .map(|(id, entry)| (*id, entry.display_name.as_str()))
        .collect::<Vec<_>>();
    entries
        .sort_by(|(a_id, a_name), (b_id, b_name)| a_name.cmp(b_name).then_with(|| a_id.cmp(b_id)));
    let mut block_type = field.block_options.block_type;
    let selected = match block_type {
        None => "Any block".to_owned(),
        Some(id) => entries
            .iter()
            .find(|(entry_id, _)| *entry_id == id)
            .map_or_else(
                || format!("Unknown type ({id})"),
                |(_, name)| (*name).to_owned(),
            ),
    };
    ui.horizontal(|ui| {
        ui.label("Block type");
        egui::ComboBox::from_id_salt(("block-type", field.id))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut block_type, None, "Any block");
                for (id, name) in entries {
                    ui.selectable_value(&mut block_type, Some(id), name);
                }
            });
    });
    if block_type != field.block_options.block_type {
        operations.push(DatabaseSchemaOperation::SetBlockOptions {
            field_id: field.id,
            options: DatabaseBlockOptions { block_type },
        });
    }
}

pub fn field_line_count(field: &DatabaseField) -> usize {
    1 + match field.field_type {
        DatabaseFieldType::Enum => field.enum_options.len() + 1,
        DatabaseFieldType::Number => 4,
        DatabaseFieldType::Block => 1,
        DatabaseFieldType::String
        | DatabaseFieldType::Boolean
        | DatabaseFieldType::Color
        | DatabaseFieldType::Datetime => 0,
    }
}

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "String",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Enum",
        DatabaseFieldType::Block => "Block",
        DatabaseFieldType::Boolean => "Boolean",
        DatabaseFieldType::Color => "Color",
        DatabaseFieldType::Datetime => "Datetime",
    }
}

fn number_scale_label(scale: DatabaseNumberScale) -> &'static str {
    match scale {
        DatabaseNumberScale::Linear => "Linear",
        DatabaseNumberScale::Logarithmic => "Logarithmic",
    }
}
