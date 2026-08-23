use block_client::{
    blocks::database_schema::{
        DatabaseEnumOption, DatabaseField, DatabaseFieldType, DatabaseSchema,
        DatabaseSchemaOperation,
    },
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_ADD, ICON_DELETE, ICON_SCHEMA},
    MaterialIcon,
};
use uuid::Uuid;

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 600.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 40.0;

impl EditorKind for DatabaseSchemaEditor {
    type Block = DatabaseSchema;

    const DISPLAY_NAME: &'static str = "Database Schema";
    const ICON: MaterialIcon = ICON_SCHEMA;

    fn open(_client: &BlockClient, block: BlockHandle<DatabaseSchema>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for DatabaseSchemaEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(DatabaseSchema::new()))
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
        ui.heading("Fields");
        ui.add_space(8.0);
        fields_ui(ui, &self.block);
        None
    }
}

pub(super) fn fields_ui(ui: &mut egui::Ui, block: &BlockHandle<DatabaseSchema>) {
    let Some(schema) = block.read() else {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return;
    };
    let fields = schema.fields().to_vec();
    drop(schema);

    let mut operations = Vec::new();
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
                        ui.selectable_value(&mut field_type, DatabaseFieldType::String, "String");
                        ui.selectable_value(&mut field_type, DatabaseFieldType::Number, "Number");
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
                    .clicked()
                {
                    operations.push(DatabaseSchemaOperation::RemoveField { field_id: field.id });
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
                options: Vec::new(),
            },
        });
    }
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
