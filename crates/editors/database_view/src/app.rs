use std::sync::Arc;

use block::{Block, BlockParent};
use block_client::block_ref::BlockRef;
use block_client::blocks::database::{Database, DatabaseOperation, DatabaseRow};
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::blocks::database_view::{
    DatabaseView, DatabaseViewKind, DatabaseViewOperation, DatabaseViewSort,
};
use block_client::references::ReferenceResolutionCache;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::database::DatabaseValueEditor;
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_DESELECT, ICON_GRID_ON, ICON_SCATTER_PLOT, ICON_SCHEMA, ICON_VIEW_KANBAN,
};
use block_editor_plugin::{egui, EditorHost};
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
    spreadsheet: SpreadsheetView,
    kanban: KanbanView,
    scatter: ScatterView,
    row_editor: RowEditor,
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

    fn operate_database(&self, operations: Vec<DatabaseOperation>) {
        let Some(database) = self.database.as_ref() else {
            return;
        };
        for operation in operations {
            database.operate(operation);
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
                options: Vec::new(),
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
        let mut operations = Vec::new();
        self.spreadsheet.formula_bar(
            ui,
            &block,
            &data.rows,
            &data.fields,
            data.sort,
            &mut operations,
        );
        self.operate_database(operations);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(block), Some(data)) =
            (self.host.clone(), self.block.clone(), self.data())
        else {
            return;
        };
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
                operations = self
                    .row_editor
                    .ui(ui, &data.rows, &data.fields, selected_row);
            });
        self.operate_database(operations);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (Some(block), Some(data)) = (self.block.clone(), self.data()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let mut operations = Vec::new();
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
        row_index: Option<usize>,
    ) -> Vec<DatabaseOperation> {
        let Some(row_index) = row_index else {
            ui.weak("Select a row to edit it here.");
            return Vec::new();
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
            return Vec::new();
        }
        self.value_editor
            .ui(ui, fields, &[row.values()], "database-view.selected-item")
            .into_iter()
            .map(|change| DatabaseOperation::SetCell {
                row_index,
                field_id: change.field_id,
                value: change.value,
            })
            .collect()
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
