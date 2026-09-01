use std::{collections::HashMap, sync::Arc};

use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::{Database, DatabaseOperation, DatabaseRow, DatabaseValue},
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
        },
        database_view::{DatabaseView, DatabaseViewKind, DatabaseViewOperation, DatabaseViewSort},
    },
    references::{ReferenceClassificationQueue, ReferenceResolutionCache},
    BlockClient, BlockHandle, ReferenceList,
};
use block_editor_plugin::{
    block_ui::{
        database::{DatabaseBlockPickRequest, DatabaseValueEditor, DatabaseValueEditorOutput},
        test_id::TestId,
        BlockLabel,
    },
    egui,
    egui_material_icons::icons::{
        ICON_DESELECT, ICON_GRID_ON, ICON_SCATTER_PLOT, ICON_SCHEMA, ICON_VIEW_KANBAN,
    },
    BlockFilter, BlockPicker, EditorHost,
};
use uuid::Uuid;

use crate::kanban::KanbanView;
use crate::scatter::ScatterView;
use crate::spreadsheet::SpreadsheetView;
use crate::{kanban, scatter, spreadsheet};

pub(crate) struct BlockRenderContext<'a> {
    pub(crate) painter: &'a egui::Painter,
    pub(crate) corners: [egui::Pos2; 4],
    pub(crate) opacity: f32,
}

struct DatabaseViewData {
    schema_id: Uuid,
    rows: Vec<DatabaseRow>,
    block_labels: HashMap<BlockRef, BlockLabel>,
    fields: Vec<DatabaseField>,
    sort: Option<DatabaseViewSort>,
    kind: DatabaseViewKind,
    kanban_field_id: Option<Uuid>,
    scatter_x_field_id: Option<Uuid>,
    scatter_y_field_id: Option<Uuid>,
}

#[derive(Default)]
pub struct DatabaseViewApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<DatabaseView>>,
    database: Option<BlockHandle<Database>>,
    schema: Option<BlockHandle<DatabaseSchema>>,
    dependencies: Option<ReferenceList>,
    spreadsheet: SpreadsheetView,
    kanban: KanbanView,
    scatter: ScatterView,
    row_editor: RowEditor,
    picker: BlockPicker,
    pending_value_target: Option<(usize, Uuid)>,
    pending_values: ReferenceClassificationQueue<(usize, Uuid)>,
    picker_error: Option<String>,
    reference_cache: ReferenceResolutionCache,
}

impl DatabaseViewApp {
    fn ensure_database(&mut self, database_ref: BlockRef) -> Option<()> {
        let client = self.client.clone()?;
        let referencing_id = self.block.as_ref()?.id();
        let database_id = self
            .reference_cache
            .resolve(&client, referencing_id, database_ref)?;
        if self
            .database
            .as_ref()
            .is_none_or(|database| database.id() != database_id)
        {
            self.database = Some(client.get_block::<Database>(database_id));
            self.dependencies =
                Some(client.watch_references(BlockReferenceList::References(database_id)));
        }
        Some(())
    }

    fn ensure_schema(&mut self, schema_ref: BlockRef) -> Option<()> {
        let client = self.client.clone()?;
        let referencing_id = self.database.as_ref()?.id();
        let schema_id = self
            .reference_cache
            .resolve(&client, referencing_id, schema_ref)?;
        if self
            .schema
            .as_ref()
            .is_none_or(|schema| schema.id() != schema_id)
        {
            self.schema = Some(client.get_block::<DatabaseSchema>(schema_id));
        }
        Some(())
    }

    fn data(&mut self) -> Option<DatabaseViewData> {
        self.poll_value_picker();
        self.reference_cache.poll();
        let view = self.block.as_ref()?.read()?;
        let database_ref = view.database_id();
        let sort = view.sort();
        let kind = view.kind();
        let kanban_field_id = view.kanban_field_id();
        let scatter_x_field_id = view.scatter_x_field_id();
        let scatter_y_field_id = view.scatter_y_field_id();
        drop(view);
        self.ensure_database(database_ref)?;
        let database = self.database.as_ref()?.read()?;
        let schema_ref = database.schema_id();
        let rows = database.rows().to_vec();
        drop(database);
        self.ensure_schema(schema_ref)?;
        let fields = self.schema.as_ref()?.read()?.fields().to_vec();
        let schema_id = self.schema.as_ref()?.id();
        let block_labels = self.block_labels(&rows);
        Some(DatabaseViewData {
            schema_id,
            rows,
            fields,
            block_labels,
            sort,
            kind,
            kanban_field_id,
            scatter_x_field_id,
            scatter_y_field_id,
        })
    }

    fn operate_database(&self, operations: Vec<DatabaseOperation>) {
        let Some(database) = self.database.as_ref() else {
            return;
        };
        for operation in operations {
            database.operate(operation);
        }
    }
    fn block_labels(&mut self, rows: &[DatabaseRow]) -> HashMap<BlockRef, BlockLabel> {
        let (Some(host), Some(client), Some(database), Some(dependencies)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.database.as_ref(),
            self.dependencies.as_ref(),
        ) else {
            return HashMap::new();
        };
        let types = host.block_types();
        let labels = dependencies
            .read()
            .into_iter()
            .map(|reference| {
                (
                    reference.id,
                    BlockLabel::for_reference(types.as_ref(), &reference),
                )
            })
            .collect::<HashMap<_, _>>();
        let referencing_id = database.id();
        rows.iter()
            .flat_map(|row| row.values().values())
            .filter_map(|value| match value {
                DatabaseValue::Block(reference) => Some(*reference),
                _ => None,
            })
            .filter_map(|reference| {
                self.reference_cache
                    .resolve(client, referencing_id, reference)
                    .and_then(|id| labels.get(&id).cloned())
                    .map(|label| (reference, label))
            })
            .collect()
    }

    fn value_target_selected(&self, target: (usize, Uuid)) -> bool {
        let Some(kind) = self
            .block
            .as_ref()
            .and_then(BlockHandle::read)
            .map(|view| view.kind())
        else {
            return false;
        };
        match kind {
            DatabaseViewKind::Spreadsheet => self.spreadsheet.selected_target() == Some(target),
            DatabaseViewKind::Kanban => self.kanban.selected_row() == Some(target.0),
            DatabaseViewKind::Scatter => self.scatter.selected_row() == Some(target.0),
        }
    }

    fn open_value_picker(
        &mut self,
        target: (usize, Uuid),
        field_name: &str,
        request: DatabaseBlockPickRequest,
    ) {
        let Some(host) = self.host.as_ref() else {
            return;
        };
        self.pending_value_target = Some(target);
        self.picker
            .open(host, value_block_filter(field_name, request));
    }

    fn poll_value_picker(&mut self) {
        let (Some(host), Some(client), Some(database)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.database.as_ref(),
        ) else {
            return;
        };
        if self
            .pending_value_target
            .is_some_and(|target| !self.value_target_selected(target))
        {
            self.pending_value_target = None;
        }
        let was_open = self.picker.is_open();
        match self.picker.poll(host) {
            Some(Ok((block_id, _))) => {
                if let Some(target) = self.pending_value_target {
                    self.pending_values
                        .push(client, database.id(), block_id, target);
                }
            }
            Some(Err(error)) => {
                self.picker_error = Some(error);
                self.pending_value_target = None;
            }
            None if was_open && !self.picker.is_open() => {
                self.pending_value_target = None;
            }
            None => {}
        }
        let (finished, failed) = self.pending_values.poll_with_failures();
        for (reference, target) in finished {
            if self.pending_value_target == Some(target) {
                database.operate(DatabaseOperation::SetCell {
                    row_index: target.0,
                    field_id: target.1,
                    value: Some(DatabaseValue::Block(reference)),
                });
                self.pending_value_target = None;
            }
        }
        if failed
            .into_iter()
            .any(|target| self.pending_value_target == Some(target))
        {
            self.picker_error = Some("Could not classify the selected block reference".to_owned());
            self.pending_value_target = None;
        }
    }

    fn selected_row(&self, kind: DatabaseViewKind) -> Option<usize> {
        match kind {
            DatabaseViewKind::Spreadsheet => self.spreadsheet.selected_row(),
            DatabaseViewKind::Kanban => self.kanban.selected_row(),
            DatabaseViewKind::Scatter => self.scatter.selected_row(),
        }
    }

    fn deselect(&mut self, kind: DatabaseViewKind) {
        match kind {
            DatabaseViewKind::Spreadsheet => self.spreadsheet.deselect(),
            DatabaseViewKind::Kanban => self.kanban.deselect(),
            DatabaseViewKind::Scatter => self.scatter.deselect(),
        }
    }

    fn view_switch(&self, ui: &mut egui::Ui, kind: DatabaseViewKind) {
        let Some(block) = self.block.as_ref() else {
            return;
        };
        ui.horizontal(|ui| {
            for (label, icon, wanted) in [
                (
                    "Spreadsheet",
                    ICON_GRID_ON.codepoint,
                    DatabaseViewKind::Spreadsheet,
                ),
                (
                    "Kanban",
                    ICON_VIEW_KANBAN.codepoint,
                    DatabaseViewKind::Kanban,
                ),
                (
                    "Scatter",
                    ICON_SCATTER_PLOT.codepoint,
                    DatabaseViewKind::Scatter,
                ),
            ] {
                if ui
                    .selectable_label(kind == wanted, format!("{icon} {label}"))
                    .test_id(&format!("database-view.kind.{label}"))
                    .clicked()
                    && kind != wanted
                {
                    block.operate(DatabaseViewOperation::SetKind { kind: wanted });
                }
            }
        });
    }
}

pub(crate) fn value_block_filter(
    field_name: &str,
    request: DatabaseBlockPickRequest,
) -> BlockFilter {
    BlockFilter {
        name: field_name.to_owned(),
        block_types: request
            .block_type
            .into_iter()
            .map(Uuid::into_bytes)
            .collect(),
        templates: false,
    }
}

impl block_editor_plugin::App for DatabaseViewApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
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
        let schema = client.create_block(DatabaseSchema::new());
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: Uuid::new_v4(),
                name: "Name".into(),
                field_type: DatabaseFieldType::String,
                enum_options: Vec::new(),
                number_options: Default::default(),
                block_options: Default::default(),
            },
        });
        let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
        schema.set_parent(BlockParent::Uuid(database.id()));
        let view = client.create_block(DatabaseView::new(BlockRef::Direct(database.id())));
        database.set_parent(BlockParent::Uuid(view.id()));
        Ok(view.id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let data = self.data()?;
        match data.kind {
            DatabaseViewKind::Spreadsheet => Some(
                self.spreadsheet
                    .intrinsic_size(data.rows.len(), &data.fields),
            ),
            DatabaseViewKind::Kanban | DatabaseViewKind::Scatter => None,
        }
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        self.reference_cache.poll();
        let Some(view) = self.block.as_ref().and_then(BlockHandle::read) else {
            return;
        };
        let database_ref = view.database_id();
        let sort = view.sort();
        let kind = view.kind();
        let kanban_field_id = view.kanban_field_id();
        let scatter_x_field_id = view.scatter_x_field_id();
        let scatter_y_field_id = view.scatter_y_field_id();
        drop(view);
        if self.ensure_database(database_ref).is_none() {
            return;
        }
        let Some(database) = self.database.as_ref().and_then(|database| database.read()) else {
            return;
        };
        let schema_ref = database.schema_id();
        let mut rows = database.rows().to_vec();
        drop(database);
        if self.ensure_schema(schema_ref).is_none() {
            return;
        }
        let Some(fields) = self
            .schema
            .as_ref()
            .and_then(|schema| schema.read())
            .map(|schema| schema.fields().to_vec())
        else {
            return;
        };
        let block_labels = self.block_labels(&rows);
        let rect = ui.max_rect();
        let context = BlockRenderContext {
            painter: ui.painter(),
            corners: [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            opacity: 1.0,
        };
        match kind {
            DatabaseViewKind::Spreadsheet => {
                if let Some(sort) = sort {
                    spreadsheet::sort_rows(&mut rows, sort, &fields, &block_labels);
                }
                spreadsheet::paint_preview(context, &rows, &fields, &block_labels);
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
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(data) = self.data() else {
            return;
        };
        let Some(block) = self.block.clone() else {
            return;
        };
        if data.kind != DatabaseViewKind::Spreadsheet {
            return;
        }
        self.spreadsheet.set_block_labels(data.block_labels.clone());
        let mut operations = Vec::new();
        let block_pick = self.spreadsheet.formula_bar(
            ui,
            &block,
            &data.rows,
            &data.fields,
            data.sort,
            &mut operations,
        );
        self.operate_database(operations);
        if let (Some(request), Some(target)) = (block_pick, self.spreadsheet.selected_target()) {
            if target.1 == request.field_id {
                if let Some(field) = data
                    .fields
                    .iter()
                    .find(|field| field.id == request.field_id)
                {
                    self.open_value_picker(target, &field.name, request);
                }
            }
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(block), Some(data)) =
            (self.host.clone(), self.block.clone(), self.data())
        else {
            return;
        };
        if let Some(error) = self.picker_error.clone() {
            ui.colored_label(ui.visuals().error_fg_color, error);
            if ui.small_button("Dismiss").clicked() {
                self.picker_error = None;
            }
            ui.separator();
        }
        egui::CollapsingHeader::new("Column settings")
            .default_open(false)
            .show(ui, |ui| {
                if ui
                    .button(format!("{} Columns", ICON_SCHEMA.codepoint))
                    .on_hover_text("Edit columns and data types")
                    .test_id("database-view.columns")
                    .clicked()
                {
                    host.open_block(data.schema_id, DatabaseSchema::TYPE_ID);
                }
            });
        ui.separator();
        let mut operations = Vec::new();
        egui::CollapsingHeader::new("View settings")
            .default_open(true)
            .show(ui, |ui| {
                self.view_switch(ui, data.kind);
                match data.kind {
                    DatabaseViewKind::Spreadsheet => {}
                    DatabaseViewKind::Kanban => {
                        kanban::status_field_picker(ui, &block, &data.fields, data.kanban_field_id);
                    }
                    DatabaseViewKind::Scatter => {
                        scatter::axis_field_pickers(
                            ui,
                            &block,
                            &data.fields,
                            data.scatter_x_field_id,
                            data.scatter_y_field_id,
                        );
                    }
                }
            });
        ui.separator();
        let mut block_pick = None;
        egui::CollapsingHeader::new("Selected item")
            .default_open(true)
            .show(ui, |ui| {
                let mut selected_row = self.selected_row(data.kind);
                if selected_row.is_some()
                    && ui
                        .button(format!("{} Deselect", ICON_DESELECT.codepoint))
                        .clicked()
                {
                    self.deselect(data.kind);
                    selected_row = None;
                }
                let output = self.row_editor.ui(
                    ui,
                    &data.rows,
                    &data.fields,
                    &data.block_labels,
                    selected_row,
                );
                operations = output
                    .changes
                    .into_iter()
                    .map(|change| DatabaseOperation::SetCell {
                        row_index: selected_row.unwrap_or_default(),
                        field_id: change.field_id,
                        value: change.value,
                    })
                    .collect();
                block_pick = output
                    .block_pick
                    .map(|request| (selected_row.unwrap_or_default(), request));
            });
        self.operate_database(operations);
        if let Some((row_index, request)) = block_pick {
            if let Some(field) = data
                .fields
                .iter()
                .find(|field| field.id == request.field_id)
            {
                self.open_value_picker((row_index, request.field_id), &field.name, request);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (Some(block), Some(data)) = (self.block.clone(), self.data()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let mut operations = Vec::new();
        self.spreadsheet.set_block_labels(data.block_labels.clone());
        self.kanban.set_block_labels(data.block_labels.clone());
        match data.kind {
            DatabaseViewKind::Spreadsheet => {
                if data.fields.is_empty() {
                    let rect = ui.available_rect_before_wrap();
                    ui.painter()
                        .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                    return;
                }
                self.spreadsheet.grid(
                    ui,
                    &block,
                    &data.rows,
                    &data.fields,
                    data.sort,
                    1.0,
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
                    return;
                };
                self.kanban.board(
                    ui,
                    &block,
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
                    return;
                };
                self.scatter.plot(ui, &data.rows, &x_field, &y_field);
            }
        }
        self.operate_database(operations);
    }
}

#[derive(Default)]
pub(crate) struct RowEditor {
    row_index: Option<usize>,
    value_editor: DatabaseValueEditor,
}

impl RowEditor {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        block_labels: &HashMap<BlockRef, BlockLabel>,
        row_index: Option<usize>,
    ) -> DatabaseValueEditorOutput {
        let Some(row_index) = row_index else {
            ui.weak("Select a row to edit it here.");
            return DatabaseValueEditorOutput::default();
        };
        if self.row_index != Some(row_index) {
            self.row_index = Some(row_index);
            self.value_editor.reset();
        }
        let empty = DatabaseRow::default();
        let row = rows.get(row_index).unwrap_or(&empty);

        ui.heading(format!("Row {}", row_index + 1));
        ui.separator();
        if fields.is_empty() {
            ui.weak("This database has no columns yet.");
            return DatabaseValueEditorOutput::default();
        }
        self.value_editor.ui(
            ui,
            fields,
            &[row.values()],
            block_labels,
            "database-view.selected-item",
        )
    }
}

pub(crate) fn paint_preview_cell(
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

pub(crate) fn preview_color(color: egui::Color32, opacity: f32) -> egui::Color32 {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    egui::Color32::from_rgba_unmultiplied(
        red,
        green,
        blue,
        (alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}
