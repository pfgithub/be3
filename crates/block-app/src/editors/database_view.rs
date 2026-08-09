mod kanban;
mod scatter;
mod spreadsheet;

use block::{Block, BlockParent};
use block_client::{
    blocks::{
        database::{Database, DatabaseOperation, DatabaseRow, DatabaseValue},
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
        },
        database_view::{DatabaseView, DatabaseViewKind, DatabaseViewOperation, DatabaseViewSort},
    },
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_DATABASE, ICON_GRID_ON, ICON_SCATTER_PLOT, ICON_SCHEMA, ICON_VIEW_KANBAN},
    MaterialIcon,
};
use uuid::Uuid;

use self::kanban::KanbanView;
use self::scatter::ScatterView;
use self::spreadsheet::SpreadsheetView;

use super::{
    BlockEditor, BlockRenderContext, CreatableEditor, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

/// The database's schema, rows, and fields, plus the view's own settings.
struct DatabaseViewData {
    schema_id: Uuid,
    rows: Vec<DatabaseRow>,
    fields: Vec<DatabaseField>,
    sort: Option<DatabaseViewSort>,
    kind: DatabaseViewKind,
    kanban_field_id: Option<Uuid>,
    scatter_x_field_id: Option<Uuid>,
    scatter_y_field_id: Option<Uuid>,
}

impl EditorKind for DatabaseEditor {
    type Block = DatabaseView;

    const DISPLAY_NAME: &'static str = "Database";
    const ICON: MaterialIcon = ICON_DATABASE;

    fn open(_client: &BlockClient, block: BlockHandle<DatabaseView>) -> Self {
        Self::new(block, None, None)
    }
}

impl CreatableEditor for DatabaseEditor {
    fn create(client: &BlockClient) -> Self {
        let schema = client.create_block(DatabaseSchema::new());
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: Uuid::new_v4(),
                name: "Name".into(),
                field_type: DatabaseFieldType::String,
                options: Vec::new(),
            },
        });
        let database = client.create_block(Database::new(schema.id()));
        schema.set_parent(BlockParent::Uuid(database.id()));
        let view = client.create_block(DatabaseView::new(database.id()));
        database.set_parent(BlockParent::Uuid(view.id()));
        Self::new(view, Some(database), Some(schema))
    }
}

pub(super) struct DatabaseEditor {
    block: BlockHandle<DatabaseView>,
    database: Option<BlockHandle<Database>>,
    schema: Option<BlockHandle<DatabaseSchema>>,
    spreadsheet: SpreadsheetView,
    kanban: KanbanView,
    scatter: ScatterView,
    row_editor: RowEditor,
}

impl DatabaseEditor {
    fn new(
        block: BlockHandle<DatabaseView>,
        database: Option<BlockHandle<Database>>,
        schema: Option<BlockHandle<DatabaseSchema>>,
    ) -> Self {
        Self {
            block,
            database,
            schema,
            spreadsheet: SpreadsheetView::default(),
            kanban: KanbanView::default(),
            scatter: ScatterView::default(),
            row_editor: RowEditor::default(),
        }
    }

    fn ensure_database(&mut self, client: &BlockClient, database_id: Uuid) {
        if self
            .database
            .as_ref()
            .is_none_or(|database| database.id() != database_id)
        {
            self.database = Some(client.get_block::<Database>(database_id));
        }
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

    fn data(&mut self, client: &BlockClient) -> Option<DatabaseViewData> {
        let view = self.block.read()?;
        let database_id = view.database_id();
        let sort = view.sort();
        let kind = view.kind();
        let kanban_field_id = view.kanban_field_id();
        let scatter_x_field_id = view.scatter_x_field_id();
        let scatter_y_field_id = view.scatter_y_field_id();
        drop(view);
        self.ensure_database(client, database_id);
        let database = self.database.as_ref()?.read()?;
        let schema_id = database.schema_id();
        let rows = database.rows().to_vec();
        drop(database);
        self.ensure_schema(client, schema_id);
        let fields = self.schema.as_ref()?.read()?.fields().to_vec();
        Some(DatabaseViewData {
            schema_id,
            rows,
            fields,
            sort,
            kind,
            kanban_field_id,
            scatter_x_field_id,
            scatter_y_field_id,
        })
    }

    /// Applies operations to the underlying database, not the view itself.
    fn operate_database(&self, operations: Vec<DatabaseOperation>) {
        let Some(database) = self.database.as_ref() else {
            return;
        };
        for operation in operations {
            database.operate(operation);
        }
    }

    /// The row shown for editing in the shared right sidebar: the spreadsheet's
    /// selected cell's row, the kanban board's selected card, or the scatter
    /// chart's selected point.
    fn selected_row(&self, kind: DatabaseViewKind) -> Option<usize> {
        match kind {
            DatabaseViewKind::Spreadsheet => self.spreadsheet.selected_row(),
            DatabaseViewKind::Kanban => self.kanban.selected_row(),
            DatabaseViewKind::Scatter => self.scatter.selected_row(),
        }
    }

    fn view_switch(&self, ui: &mut egui::Ui, kind: DatabaseViewKind) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    kind == DatabaseViewKind::Spreadsheet,
                    format!("{} Spreadsheet", ICON_GRID_ON.codepoint),
                )
                .clicked()
                && kind != DatabaseViewKind::Spreadsheet
            {
                self.block.operate(DatabaseViewOperation::SetKind {
                    kind: DatabaseViewKind::Spreadsheet,
                });
            }
            if ui
                .selectable_label(
                    kind == DatabaseViewKind::Kanban,
                    format!("{} Kanban", ICON_VIEW_KANBAN.codepoint),
                )
                .clicked()
                && kind != DatabaseViewKind::Kanban
            {
                self.block.operate(DatabaseViewOperation::SetKind {
                    kind: DatabaseViewKind::Kanban,
                });
            }
            if ui
                .selectable_label(
                    kind == DatabaseViewKind::Scatter,
                    format!("{} Scatter", ICON_SCATTER_PLOT.codepoint),
                )
                .clicked()
                && kind != DatabaseViewKind::Scatter
            {
                self.block.operate(DatabaseViewOperation::SetKind {
                    kind: DatabaseViewKind::Scatter,
                });
            }
        });
    }
}

impl BlockEditor for DatabaseEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let Some(view) = self.block.read() else {
            return false;
        };
        let database_id = view.database_id();
        let sort = view.sort();
        let kind = view.kind();
        let kanban_field_id = view.kanban_field_id();
        let scatter_x_field_id = view.scatter_x_field_id();
        let scatter_y_field_id = view.scatter_y_field_id();
        drop(view);
        self.ensure_database(editors.client(), database_id);
        let Some(database) = self.database.as_ref().and_then(|database| database.read()) else {
            return false;
        };
        let schema_id = database.schema_id();
        let mut rows = database.rows().to_vec();
        drop(database);
        self.ensure_schema(editors.client(), schema_id);
        let Some(fields) = self
            .schema
            .as_ref()
            .and_then(|schema| schema.read())
            .map(|schema| schema.fields().to_vec())
        else {
            return false;
        };
        match kind {
            DatabaseViewKind::Spreadsheet => {
                if let Some(sort) = sort {
                    spreadsheet::sort_rows(&mut rows, sort, &fields);
                }
                spreadsheet::paint_preview(context, &rows, &fields);
            }
            DatabaseViewKind::Kanban => {
                kanban::paint_preview(context, &rows, &fields, kanban_field_id);
            }
            DatabaseViewKind::Scatter => {
                scatter::paint_preview(
                    context,
                    &rows,
                    &fields,
                    scatter_x_field_id,
                    scatter_y_field_id,
                );
            }
        }
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        DirectEditorInteraction::Live
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let data = self.data(editors.client())?;
        match data.kind {
            DatabaseViewKind::Spreadsheet => Some(
                self.spreadsheet
                    .intrinsic_size(data.rows.len(), &data.fields),
            ),
            DatabaseViewKind::Kanban | DatabaseViewKind::Scatter => None,
        }
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let data = self.data(editors.client())?;
        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Columns", ICON_SCHEMA.codepoint))
                .on_hover_text("Edit columns and data types")
                .clicked()
            {
                action = Some(EditorAction::OpenBlock {
                    id: data.schema_id,
                    block_type: DatabaseSchema::TYPE_ID,
                });
            }
            ui.separator();
            self.view_switch(ui, data.kind);
        });
        ui.separator();
        let mut operations = Vec::new();
        match data.kind {
            DatabaseViewKind::Spreadsheet => {
                self.spreadsheet.formula_bar(
                    ui,
                    &self.block,
                    &data.rows,
                    &data.fields,
                    data.sort,
                    &mut operations,
                );
            }
            DatabaseViewKind::Kanban => {
                kanban::status_field_picker(ui, &self.block, &data.fields, data.kanban_field_id);
            }
            DatabaseViewKind::Scatter => {
                scatter::axis_field_pickers(
                    ui,
                    &self.block,
                    &data.fields,
                    data.scatter_x_field_id,
                    data.scatter_y_field_id,
                );
            }
        }
        self.operate_database(operations);
        action
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let data = self.data(editors.client())?;
        let selected_row = self.selected_row(data.kind);
        let operations = self
            .row_editor
            .ui(ui, &data.rows, &data.fields, selected_row);
        self.operate_database(operations);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let data = self.data(editors.client())?;
        let mut operations = Vec::new();
        match data.kind {
            DatabaseViewKind::Spreadsheet => {
                if data.fields.is_empty() {
                    let rect = ui.available_rect_before_wrap();
                    ui.painter()
                        .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                    return None;
                }
                self.spreadsheet.grid(
                    ui,
                    &self.block,
                    &data.rows,
                    &data.fields,
                    data.sort,
                    scale,
                    &mut operations,
                );
            }
            DatabaseViewKind::Kanban => {
                let status_field = data
                    .kanban_field_id
                    .and_then(|id| data.fields.iter().find(|field| field.id == id).cloned());
                let Some(status_field) = status_field else {
                    ui.centered_and_justified(|ui| {
                        ui.weak("Choose a status field above to use the kanban view.");
                    });
                    return None;
                };
                self.kanban.board(
                    ui,
                    &self.block,
                    &data.rows,
                    &data.fields,
                    &status_field,
                    &mut operations,
                );
            }
            DatabaseViewKind::Scatter => {
                let axis_fields = data.scatter_x_field_id.and_then(|x_id| {
                    let x_field = data.fields.iter().find(|field| field.id == x_id)?;
                    let y_field = data
                        .scatter_y_field_id
                        .and_then(|y_id| data.fields.iter().find(|field| field.id == y_id))?;
                    Some((x_field.clone(), y_field.clone()))
                });
                let Some((x_field, y_field)) = axis_fields else {
                    ui.centered_and_justified(|ui| {
                        ui.weak("Choose X and Y fields above to use the scatter view.");
                    });
                    return None;
                };
                self.scatter.plot(ui, &data.rows, &x_field, &y_field);
            }
        }
        self.operate_database(operations);
        None
    }
}

/// Editing surface for a single row, shared between every view kind: the
/// spreadsheet shows it for the selected cell's row, the kanban board for
/// the selected card.
#[derive(Default)]
struct RowEditor {
    row_index: Option<usize>,
    buffers: std::collections::BTreeMap<Uuid, String>,
}

impl RowEditor {
    /// Resets the edit buffers when the selected row changes, and fills in
    /// any missing ones from the row's current values. Buffers otherwise
    /// persist across frames so in-progress typing (e.g. a lone "-" in a
    /// number field) is not clobbered by the freshly read value.
    fn sync(&mut self, row_index: usize, row: &DatabaseRow, fields: &[DatabaseField]) {
        if self.row_index != Some(row_index) {
            self.row_index = Some(row_index);
            self.buffers.clear();
        }
        self.buffers
            .retain(|field_id, _| fields.iter().any(|field| field.id == *field_id));
        for field in fields {
            self.buffers
                .entry(field.id)
                .or_insert_with(|| cell_text(row, field));
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        row_index: Option<usize>,
    ) -> Vec<DatabaseOperation> {
        let mut operations = Vec::new();
        let Some(row_index) = row_index else {
            ui.weak("Select a row to edit it here.");
            return operations;
        };
        let empty = DatabaseRow::default();
        let row = rows.get(row_index).unwrap_or(&empty);
        self.sync(row_index, row, fields);

        ui.heading(format!("Row {}", row_index + 1));
        ui.separator();
        if fields.is_empty() {
            ui.weak("This database has no columns yet.");
            return operations;
        }
        for field in fields {
            ui.label(format!(
                "{} ({})",
                field.name,
                field_type_label(field.field_type)
            ));
            match field.field_type {
                DatabaseFieldType::String | DatabaseFieldType::Number => {
                    let buffer = self.buffers.entry(field.id).or_default();
                    let response =
                        ui.add(egui::TextEdit::singleline(buffer).desired_width(f32::INFINITY));
                    if response.changed() {
                        let value = if field.field_type == DatabaseFieldType::Number
                            && buffer.trim().is_empty()
                        {
                            Some(None)
                        } else {
                            parse_cell_value(buffer, field).map(Some)
                        };
                        if let Some(value) = value {
                            operations.push(DatabaseOperation::SetCell {
                                row_index,
                                field_id: field.id,
                                value,
                            });
                        }
                    }
                }
                DatabaseFieldType::Enum => {
                    let current = row.value(field.id).and_then(|value| match value {
                        DatabaseValue::Enum(id) => Some(*id),
                        _ => None,
                    });
                    let current_name = current
                        .and_then(|id| field.options.iter().find(|option| option.id == id))
                        .map_or("None", |option| option.name.as_str());
                    egui::ComboBox::new(("database-row-editor-enum", row_index, field.id), "")
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current.is_none(), "None").clicked() {
                                operations.push(DatabaseOperation::SetCell {
                                    row_index,
                                    field_id: field.id,
                                    value: None,
                                });
                            }
                            for option in &field.options {
                                if ui
                                    .selectable_label(current == Some(option.id), &option.name)
                                    .clicked()
                                {
                                    operations.push(DatabaseOperation::SetCell {
                                        row_index,
                                        field_id: field.id,
                                        value: Some(DatabaseValue::Enum(option.id)),
                                    });
                                }
                            }
                        });
                }
            }
            ui.add_space(8.0);
        }
        operations
    }
}

fn cell_text(row: &DatabaseRow, field: &DatabaseField) -> String {
    match row.value(field.id) {
        Some(value) => database_value_text(value, field),
        None => String::new(),
    }
}

fn database_value_text(value: &DatabaseValue, field: &DatabaseField) -> String {
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

fn parse_cell_value(value: &str, field: &DatabaseField) -> Option<DatabaseValue> {
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

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "Text",
        DatabaseFieldType::Number => "Number",
        DatabaseFieldType::Enum => "Enum",
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_preview_cell(
    painter: &egui::Painter,
    rect: egui::Rect,
    fill: egui::Color32,
    stroke: egui::Stroke,
    text: &str,
    alignment: egui::Align2,
    text_color: egui::Color32,
    font: egui::FontId,
    inset: f32,
) {
    painter.rect(rect, 0.0, fill, stroke, egui::StrokeKind::Inside);
    let position = match alignment {
        egui::Align2::LEFT_CENTER => rect.left_center() + egui::vec2(inset, 0.0),
        egui::Align2::RIGHT_CENTER => rect.right_center() - egui::vec2(inset, 0.0),
        _ => rect.center(),
    };
    painter
        .with_clip_rect(rect.shrink(1.0))
        .text(position, alignment, text, font, text_color);
}

fn preview_color(color: egui::Color32, opacity: f32) -> egui::Color32 {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    egui::Color32::from_rgba_unmultiplied(
        red,
        green,
        blue,
        (alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}
