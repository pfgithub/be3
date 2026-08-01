use block::{Block, BlockParent};
use block_client::{
    blocks::database_schema::{
        DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
    },
    BlockHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::icons::{ICON_ADD, ICON_DELETE, ICON_SCHEMA};
use uuid::Uuid;

use super::{BlockEditor, EditorAccess, EditorAction, EditorRegistration};

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
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        DatabaseSchema::TYPE_ID
    }

    fn name(&self) -> String {
        self.block.name()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.block.read().map(|_| self.block.relationships())
    }

    fn set_parent(&self, parent: BlockParent) {
        self.block.set_parent(parent);
    }

    fn note_backref(&self, id: Uuid) {
        self.block.note_backref(id);
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
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
