use std::sync::Arc;

use block_client::blocks::database_schema::{
    DatabaseEnumOption, DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{ICON_ADD, ICON_DELETE};
use block_editor_plugin::{egui, EditorHost};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 600.0;
const ROW_HEIGHT: f32 = 40.0;

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
        let fields = self.block.as_ref()?.read()?.fields().len();
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            ROW_HEIGHT * (fields + 2) as f32,
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
        fields_ui(ui, &block, editable);
    }
}

pub fn fields_ui(ui: &mut egui::Ui, block: &BlockHandle<DatabaseSchema>, editable: bool) {
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
        egui::Grid::new(("database-schema", block.id()))
            .num_columns(4)
            .striped(true)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Type");
                ui.strong("Options");
                ui.end_row();

                for field in fields {
                    let mut name = field.name.clone();
                    if ui
                        .text_edit_singleline(&mut name)
                        .test_id(&format!("database-schema.name.{}", field.id))
                        .changed()
                    {
                        operations.push(DatabaseSchemaOperation::RenameField {
                            field_id: field.id,
                            name,
                        });
                    }

                    let mut field_type = field.field_type;
                    egui::ComboBox::from_id_salt(("field-type", field.id))
                        .selected_text(field_type_label(field_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut field_type,
                                DatabaseFieldType::String,
                                "String",
                            );
                            ui.selectable_value(
                                &mut field_type,
                                DatabaseFieldType::Number,
                                "Number",
                            );
                            ui.selectable_value(&mut field_type, DatabaseFieldType::Enum, "Enum");
                        });
                    if field_type != field.field_type {
                        operations.push(DatabaseSchemaOperation::SetFieldType {
                            field_id: field.id,
                            field_type,
                        });
                    }

                    if field.field_type == DatabaseFieldType::Enum {
                        enum_options_ui(ui, &field, &mut operations);
                    } else {
                        ui.label("");
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
                    ui.end_row();
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
                    options: Vec::new(),
                },
            });
        }
    });
    for operation in operations {
        block.operate(operation);
    }
}

fn enum_options_ui(
    ui: &mut egui::Ui,
    field: &DatabaseField,
    operations: &mut Vec<DatabaseSchemaOperation>,
) {
    ui.horizontal(|ui| {
        for option in &field.options {
            let mut name = option.name.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut name).desired_width(80.0))
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
                .clicked()
            {
                operations.push(DatabaseSchemaOperation::RemoveEnumOption {
                    field_id: field.id,
                    option_id: option.id,
                });
            }
        }
        if ui
            .small_button(ICON_ADD)
            .on_hover_text("Add option")
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
    });
}

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "String",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Enum",
    }
}
