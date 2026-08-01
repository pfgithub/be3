use block::{Block, BlockParent};
use block_client::{
    blocks::{
        database::{Database, DatabaseOperation, DatabaseRow, DatabaseValue},
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
        },
    },
    BlockClient, BlockHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::icons::{ICON_ADD, ICON_DATABASE, ICON_DELETE, ICON_SCHEMA};
use uuid::Uuid;

use super::{BlockEditor, EditorAccess, EditorAction, EditorRegistration};

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: Database::TYPE_ID,
        display_name: "Database",
        icon: ICON_DATABASE,
        create: Some(|client| {
            let schema = client.create_block(DatabaseSchema::new());
            schema.operate(DatabaseSchemaOperation::AddField {
                field: DatabaseField {
                    id: Uuid::new_v4(),
                    name: "Name".into(),
                    field_type: DatabaseFieldType::String,
                },
            });
            let database = client.create_block(Database::new(schema.id()));
            schema.note_backref(database.id());
            schema.set_parent(BlockParent::Uuid(database.id()));
            Box::new(DatabaseEditor::new(database, Some(schema)))
        }),
        open: |client, id| Box::new(DatabaseEditor::new(client.get_block::<Database>(id), None)),
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

struct DatabaseEditor {
    block: BlockHandle<Database>,
    schema: Option<BlockHandle<DatabaseSchema>>,
}

impl DatabaseEditor {
    fn new(block: BlockHandle<Database>, schema: Option<BlockHandle<DatabaseSchema>>) -> Self {
        Self { block, schema }
    }

    fn ensure_schema(&mut self, client: &BlockClient, schema_id: Uuid) {
        if self
            .schema
            .as_ref()
            .is_none_or(|schema| schema.id() != schema_id)
        {
            self.schema = Some(client.get_block::<DatabaseSchema>(schema_id));
        }
    }
}

impl BlockEditor for DatabaseEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        Database::TYPE_ID
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
        editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        let Some(database) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let schema_id = database.schema_id();
        let rows = database.rows().to_vec();
        drop(database);

        self.ensure_schema(editors.client(), schema_id);
        let Some(schema_handle) = &self.schema else {
            return None;
        };
        let Some(schema) = schema_handle.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let fields = schema.fields().to_vec();
        drop(schema);

        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Edit schema", ICON_SCHEMA.codepoint))
                .clicked()
            {
                if schema_id != self.block.id() {
                    editors.ensure(schema_id, DatabaseSchema::TYPE_ID);
                }
                action = Some(EditorAction::OpenBlock {
                    id: schema_id,
                    block_type: DatabaseSchema::TYPE_ID,
                });
            }
            if ui
                .button(format!("{} Add row", ICON_ADD.codepoint))
                .clicked()
            {
                self.block.operate(DatabaseOperation::AddRow {
                    row: DatabaseRow::new(Uuid::new_v4()),
                });
            }
        });
        ui.separator();

        if fields.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This database has no fields. Add fields in its schema.");
            });
            return action;
        }

        let mut operations = Vec::new();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new(("database-grid", self.block.id()))
                    .num_columns(fields.len() + 1)
                    .striped(true)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        for field in &fields {
                            ui.strong(&field.name).on_hover_text(format!(
                                "{} · {}",
                                field_type_label(field.field_type),
                                field.id
                            ));
                        }
                        ui.label("");
                        ui.end_row();

                        for row in &rows {
                            for field in &fields {
                                cell_editor(ui, row, field, &mut operations);
                            }
                            if ui
                                .small_button(ICON_DELETE)
                                .on_hover_text("Delete row")
                                .clicked()
                            {
                                operations.push(DatabaseOperation::RemoveRow { row_id: row.id });
                            }
                            ui.end_row();
                        }
                    });
            });
        for operation in operations {
            self.block.operate(operation);
        }
        action
    }
}

fn cell_editor(
    ui: &mut egui::Ui,
    row: &DatabaseRow,
    field: &DatabaseField,
    operations: &mut Vec<DatabaseOperation>,
) {
    let changed_value = match field.field_type {
        DatabaseFieldType::String => {
            let mut value = match row.value(field.id) {
                Some(DatabaseValue::String(value)) => value.clone(),
                _ => String::new(),
            };
            ui.add_sized([160.0, 24.0], egui::TextEdit::singleline(&mut value))
                .changed()
                .then_some(DatabaseValue::String(value))
        }
        DatabaseFieldType::Number => {
            let mut value = match row.value(field.id) {
                Some(DatabaseValue::Number(value)) => *value,
                _ => 0.0,
            };
            ui.add_sized([120.0, 24.0], egui::DragValue::new(&mut value))
                .changed()
                .then_some(DatabaseValue::Number(value))
        }
    };
    if let Some(value) = changed_value {
        operations.push(DatabaseOperation::SetCell {
            row_id: row.id,
            field_id: field.id,
            value: Some(value),
        });
    }
}

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "String",
        DatabaseFieldType::Number => "Number",
    }
}
