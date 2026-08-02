use block::Block;
use block_client::{
    blocks::database_schema::{
        DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
    },
    BlockHandle,
};
use eframe::egui;
use egui_material_icons::icons::{ICON_ADD, ICON_DELETE, ICON_SCHEMA};
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorRegistration,
};

const DIRECT_EDITOR_WIDTH: f32 = 600.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 40.0;

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: DatabaseSchema::TYPE_ID,
        display_name: "Database Schema",
        icon: ICON_SCHEMA,
        create: Some(|client| {
            Box::new(DatabaseSchemaEditor::new(
                client.create_block(DatabaseSchema::new()),
            ))
        }),
        open: |client, id| {
            Box::new(DatabaseSchemaEditor::new(
                client.get_block::<DatabaseSchema>(id),
            ))
        },
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

pub(super) struct DatabaseSchemaEditor {
    block: BlockHandle<DatabaseSchema>,
}

impl DatabaseSchemaEditor {
    pub(super) fn new(block: BlockHandle<DatabaseSchema>) -> Self {
        Self { block }
    }
}

impl BlockEditor for DatabaseSchemaEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let field_count = self.block.read()?.fields().len();
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_ROW_HEIGHT * (field_count + 2) as f32,
        ))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(schema) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let fields = schema.fields().to_vec();
        drop(schema);

        ui.heading("Fields");
        ui.add_space(8.0);
        let mut operations = Vec::new();
        egui::Grid::new(("database-schema", self.block.id()))
            .num_columns(3)
            .striped(true)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Type");
                ui.end_row();

                for field in fields {
                    let mut name = field.name.clone();
                    if ui.text_edit_singleline(&mut name).changed() {
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
                        });
                    if field_type != field.field_type {
                        operations.push(DatabaseSchemaOperation::SetFieldType {
                            field_id: field.id,
                            field_type,
                        });
                    }

                    if ui
                        .small_button(ICON_DELETE)
                        .on_hover_text("Delete field")
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
            .clicked()
        {
            operations.push(DatabaseSchemaOperation::AddField {
                field: DatabaseField {
                    id: Uuid::new_v4(),
                    name: "Field".into(),
                    field_type: DatabaseFieldType::String,
                },
            });
        }
        for operation in operations {
            self.block.operate(operation);
        }
        None
    }
}

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "String",
        DatabaseFieldType::Number => "Number",
    }
}
