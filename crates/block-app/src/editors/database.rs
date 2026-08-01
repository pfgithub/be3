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

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, EditorAction, EditorRegistration,
};

const ROW_HEADER_WIDTH: f32 = 44.0;
const ROW_HEIGHT: f32 = 28.0;
const STRING_COLUMN_WIDTH: f32 = 180.0;
const NUMBER_COLUMN_WIDTH: f32 = 120.0;

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

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellAddress {
    row_id: Uuid,
    field_id: Uuid,
}

struct DatabaseEditor {
    block: BlockHandle<Database>,
    schema: Option<BlockHandle<DatabaseSchema>>,
    selected: Option<CellAddress>,
    editing: Option<CellAddress>,
    edit_buffer: String,
    edit_original: Option<DatabaseValue>,
    request_edit_focus: bool,
}

impl DatabaseEditor {
    fn new(block: BlockHandle<Database>, schema: Option<BlockHandle<DatabaseSchema>>) -> Self {
        Self {
            block,
            schema,
            selected: None,
            editing: None,
            edit_buffer: String::new(),
            edit_original: None,
            request_edit_focus: false,
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

    fn data(
        &mut self,
        client: &BlockClient,
    ) -> Option<(Uuid, Vec<DatabaseRow>, Vec<DatabaseField>)> {
        let database = self.block.read()?;
        let schema_id = database.schema_id();
        let rows = database.rows().to_vec();
        drop(database);
        self.ensure_schema(client, schema_id);
        let fields = self.schema.as_ref()?.read()?.fields().to_vec();
        Some((schema_id, rows, fields))
    }

    fn begin_edit(
        &mut self,
        row: &DatabaseRow,
        field: &DatabaseField,
        replacement: Option<String>,
    ) {
        let address = CellAddress {
            row_id: row.id,
            field_id: field.id,
        };
        self.selected = Some(address);
        self.editing = Some(address);
        self.edit_original = row.value(field.id).cloned();
        self.edit_buffer = replacement.unwrap_or_else(|| cell_text(row, field));
        self.request_edit_focus = true;
    }

    fn finish_edit(&mut self) {
        self.editing = None;
        self.edit_original = None;
        self.request_edit_focus = false;
    }

    fn select_cell(&mut self, row: &DatabaseRow, field: &DatabaseField) {
        self.finish_edit();
        self.selected = Some(CellAddress {
            row_id: row.id,
            field_id: field.id,
        });
        self.edit_buffer = cell_text(row, field);
    }

    fn move_selection(
        &mut self,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        dx: isize,
        dy: isize,
    ) {
        let Some(selected) = self.selected else {
            if let (Some(row), Some(field)) = (rows.first(), fields.first()) {
                self.selected = Some(CellAddress {
                    row_id: row.id,
                    field_id: field.id,
                });
            }
            return;
        };
        let Some(row_index) = rows.iter().position(|row| row.id == selected.row_id) else {
            self.selected = None;
            return;
        };
        let Some(field_index) = fields
            .iter()
            .position(|field| field.id == selected.field_id)
        else {
            self.selected = None;
            return;
        };
        let row_index = row_index.saturating_add_signed(dy).min(rows.len() - 1);
        let field_index = field_index.saturating_add_signed(dx).min(fields.len() - 1);
        self.select_cell(&rows[row_index], &fields[field_index]);
    }

    fn handle_keyboard(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        operations: &mut Vec<DatabaseOperation>,
    ) {
        if rows.is_empty() || fields.is_empty() || self.editing.is_some() {
            return;
        }
        let focus_id = grid_focus_id(self.block.id());
        ui.interact(
            egui::Rect::from_min_size(ui.next_widget_position(), egui::Vec2::ZERO),
            focus_id,
            egui::Sense::focusable_noninteractive(),
        );
        if !ui.memory(|memory| memory.has_focus(focus_id)) {
            return;
        }
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                focus_id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
            memory.move_focus(egui::FocusDirection::None);
        });

        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)) {
            self.move_selection(rows, fields, -1, 0);
        } else if ui
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
        {
            self.move_selection(rows, fields, 1, 0);
        } else if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
        {
            self.move_selection(rows, fields, 0, -1);
        } else if ui
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            self.move_selection(rows, fields, 0, 1);
        } else {
            let backwards =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
            let forwards =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
            if backwards || forwards {
                self.move_selection(rows, fields, if backwards { -1 } else { 1 }, 0);
            }
        }

        let Some(selected) = self.selected else {
            return;
        };
        let Some(row) = rows.iter().find(|row| row.id == selected.row_id) else {
            return;
        };
        let Some(field) = fields.iter().find(|field| field.id == selected.field_id) else {
            return;
        };

        if ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::F2)
        }) {
            self.begin_edit(row, field, None);
        } else if ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
        }) {
            operations.push(DatabaseOperation::SetCell {
                row_id: row.id,
                field_id: field.id,
                value: None,
            });
        } else if let Some(text) = take_typed_text(ui) {
            if let Some(value) = parse_cell_value(&text, field.field_type) {
                operations.push(DatabaseOperation::SetCell {
                    row_id: row.id,
                    field_id: field.id,
                    value: Some(value),
                });
            }
            self.begin_edit(row, field, Some(text));
        }
    }

    fn formula_bar(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        operations: &mut Vec<DatabaseOperation>,
    ) {
        let selection = self.selected.and_then(|selected| {
            let row_index = rows.iter().position(|row| row.id == selected.row_id)?;
            let field_index = fields
                .iter()
                .position(|field| field.id == selected.field_id)?;
            Some((selected, row_index, field_index))
        });
        let cell_label = selection.map_or_else(String::new, |(_, row_index, field_index)| {
            format!("{}{}", column_name(field_index), row_index + 1)
        });
        let formula_id = formula_input_id(self.block.id());
        if self.request_edit_focus {
            ui.memory_mut(|memory| memory.request_focus(formula_id));
            self.request_edit_focus = false;
        }

        ui.horizontal(|ui| {
            ui.add_sized(
                [ROW_HEADER_WIDTH, ROW_HEIGHT],
                egui::Label::new(egui::RichText::new(cell_label).strong()),
            );
            let response = ui.add_enabled(
                selection.is_some(),
                egui::TextEdit::singleline(&mut self.edit_buffer)
                    .id(formula_id)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY)
                    .hint_text("Select a cell"),
            );

            let Some((address, row_index, field_index)) = selection else {
                return;
            };
            let row = &rows[row_index];
            let field = &fields[field_index];
            if response.gained_focus() && self.editing.is_none() {
                self.editing = Some(address);
                self.edit_original = row.value(field.id).cloned();
            }
            if response.changed() {
                let value = if field.field_type == DatabaseFieldType::Number
                    && self.edit_buffer.trim().is_empty()
                {
                    Some(None)
                } else {
                    parse_cell_value(&self.edit_buffer, field.field_type).map(Some)
                };
                if let Some(value) = value {
                    operations.push(DatabaseOperation::SetCell {
                        row_id: row.id,
                        field_id: field.id,
                        value,
                    });
                }
            }
            if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Escape))
            {
                operations.push(DatabaseOperation::SetCell {
                    row_id: row.id,
                    field_id: field.id,
                    value: self.edit_original.clone(),
                });
                self.edit_buffer = self
                    .edit_original
                    .as_ref()
                    .map_or_else(String::new, database_value_text);
                self.finish_edit();
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(self.block.id())));
            } else if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                self.finish_edit();
                self.move_selection(rows, fields, 0, 1);
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(self.block.id())));
            } else if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Tab))
            {
                let backwards = ui.input(|input| input.modifiers.shift);
                self.finish_edit();
                self.move_selection(rows, fields, if backwards { -1 } else { 1 }, 0);
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(self.block.id())));
            }
        });
    }

    fn cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &DatabaseRow,
        field: &DatabaseField,
        scale: f32,
    ) {
        let address = CellAddress {
            row_id: row.id,
            field_id: field.id,
        };
        let selected = self.selected == Some(address);
        let size = egui::vec2(column_width(field.field_type), ROW_HEIGHT) * scale;
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        paint_cell_background(ui, rect, selected);
        paint_cell_text(ui, rect, &cell_text(row, field), field.field_type, scale);
        if response.clicked() {
            self.select_cell(row, field);
            ui.memory_mut(|memory| memory.request_focus(grid_focus_id(self.block.id())));
        }
        if response.double_clicked() {
            self.begin_edit(row, field, None);
        }
    }

    fn controls(
        &mut self,
        ui: &mut egui::Ui,
        schema_id: Uuid,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
    ) -> Option<EditorAction> {
        let mut action = None;
        let mut operations = Vec::new();
        ui.horizontal(|ui| {
            if ui.button(format!("{} Row", ICON_ADD.codepoint)).clicked() {
                let row_id = Uuid::new_v4();
                operations.push(DatabaseOperation::AddRow {
                    row: DatabaseRow::new(row_id),
                });
                if let Some(field) = fields.first() {
                    self.selected = Some(CellAddress {
                        row_id,
                        field_id: field.id,
                    });
                    self.edit_buffer.clear();
                    ui.memory_mut(|memory| memory.request_focus(grid_focus_id(self.block.id())));
                }
            }
            let selected_row = self.selected.map(|cell| cell.row_id);
            if ui
                .add_enabled(
                    selected_row.is_some(),
                    egui::Button::new(format!("{} Row", ICON_DELETE.codepoint)),
                )
                .on_hover_text("Delete selected row")
                .clicked()
            {
                operations.push(DatabaseOperation::RemoveRow {
                    row_id: selected_row.expect("button is only enabled with a selected row"),
                });
                self.selected = None;
                self.finish_edit();
            }
            ui.separator();
            if ui
                .button(format!("{} Columns", ICON_SCHEMA.codepoint))
                .on_hover_text("Edit columns and data types")
                .clicked()
            {
                action = Some(EditorAction::OpenBlock {
                    id: schema_id,
                    block_type: DatabaseSchema::TYPE_ID,
                });
            }
        });
        ui.separator();
        self.formula_bar(ui, rows, fields, &mut operations);
        for operation in operations {
            self.block.operate(operation);
        }
        action
    }

    fn grid(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        scale: f32,
    ) {
        let mut operations = Vec::new();
        self.handle_keyboard(ui, rows, fields, &mut operations);
        egui::Grid::new(("database-grid", self.block.id()))
            .num_columns(fields.len() + 1)
            .spacing(egui::Vec2::ZERO)
            .show(ui, |ui| {
                spreadsheet_header(
                    ui,
                    egui::vec2(ROW_HEADER_WIDTH, ROW_HEIGHT) * scale,
                    "",
                    scale,
                );
                for (column_index, field) in fields.iter().enumerate() {
                    let response = spreadsheet_header(
                        ui,
                        egui::vec2(column_width(field.field_type), ROW_HEIGHT) * scale,
                        &field.name,
                        scale,
                    );
                    response.on_hover_text(format!(
                        "Column {} · {}",
                        column_name(column_index),
                        field_type_label(field.field_type),
                    ));
                }
                ui.end_row();

                for (row_index, row) in rows.iter().enumerate() {
                    let selected = self.selected.is_some_and(|cell| cell.row_id == row.id);
                    spreadsheet_row_header(ui, row_index + 1, selected, scale);
                    for field in fields {
                        self.cell_editor(ui, row, field, scale);
                    }
                    ui.end_row();
                }
            });
        for operation in operations {
            self.block.operate(operation);
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

    fn render(
        &mut self,
        context: BlockRenderContext<'_>,
        _editors: &mut super::EditorAccess<'_>,
    ) -> bool {
        let Some(database) = self.block.read() else {
            return false;
        };
        let rows = database.rows().to_vec();
        drop(database);
        let Some(fields) = self
            .schema
            .as_ref()
            .and_then(|schema| schema.read())
            .map(|schema| schema.fields().to_vec())
        else {
            return false;
        };
        paint_database_preview(context, &rows, &fields);
        true
    }

    fn direct_editor_capabilities(&self) -> Option<DirectEditorCapabilities> {
        Some(DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: false,
        })
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        editors: &mut super::EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let (_, rows, fields) = self.data(editors.client())?;
        Some(egui::vec2(
            ROW_HEADER_WIDTH
                + fields
                    .iter()
                    .map(|field| column_width(field.field_type))
                    .sum::<f32>(),
            ROW_HEIGHT * (rows.len() + 1) as f32,
        ))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut super::EditorAccess<'_>,
        _viewport: &mut super::DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (schema_id, rows, fields) = self.data(editors.client())?;
        self.controls(ui, schema_id, &rows, &fields)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut super::EditorAccess<'_>,
        scale: f32,
        _viewport: &mut super::DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (_, rows, fields) = self.data(editors.client())?;
        if fields.is_empty() {
            let rect = ui.available_rect_before_wrap();
            ui.painter()
                .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
            return None;
        }
        self.grid(ui, &rows, &fields, scale);
        None
    }
}

fn spreadsheet_header(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    text: &str,
    scale: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let visuals = ui.visuals();
    ui.painter().rect(
        rect,
        0.0,
        visuals.faint_bg_color,
        visuals.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.painter().with_clip_rect(rect).text(
        rect.left_center() + egui::vec2(8.0 * scale, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional((14.0 * scale).max(6.0)),
        visuals.strong_text_color(),
    );
    response
}

fn spreadsheet_row_header(ui: &mut egui::Ui, number: usize, selected: bool, scale: f32) {
    let size = egui::vec2(ROW_HEADER_WIDTH, ROW_HEIGHT) * scale;
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else {
        visuals.faint_bg_color
    };
    ui.painter().rect(
        rect,
        0.0,
        fill,
        visuals.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        number,
        egui::FontId::proportional((12.0 * scale).max(6.0)),
        visuals.text_color(),
    );
}

fn paint_cell_background(ui: &egui::Ui, rect: egui::Rect, selected: bool) {
    let visuals = ui.visuals();
    ui.painter().rect(
        rect,
        0.0,
        visuals.extreme_bg_color,
        visuals.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    if selected {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, visuals.selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
}

fn paint_cell_text(
    ui: &egui::Ui,
    rect: egui::Rect,
    text: &str,
    field_type: DatabaseFieldType,
    scale: f32,
) {
    let (position, alignment) = match field_type {
        DatabaseFieldType::String => (
            rect.left_center() + egui::vec2(6.0 * scale, 0.0),
            egui::Align2::LEFT_CENTER,
        ),
        DatabaseFieldType::Number => (
            rect.right_center() - egui::vec2(6.0 * scale, 0.0),
            egui::Align2::RIGHT_CENTER,
        ),
    };
    ui.painter().with_clip_rect(rect.shrink(2.0)).text(
        position,
        alignment,
        text,
        egui::FontId::proportional((14.0 * scale).max(6.0)),
        ui.visuals().text_color(),
    );
}

fn paint_database_preview(
    context: BlockRenderContext<'_>,
    rows: &[DatabaseRow],
    fields: &[DatabaseField],
) {
    let rect = egui::Rect::from_min_max(context.corners[0], context.corners[2]);
    let intrinsic_width = ROW_HEADER_WIDTH
        + fields
            .iter()
            .map(|field| column_width(field.field_type))
            .sum::<f32>();
    let scale = (rect.width() / intrinsic_width.max(1.0)).max(0.01);
    let row_height = ROW_HEIGHT * scale;
    let row_header_width = ROW_HEADER_WIDTH * scale;
    let visuals = &context.painter.ctx().global_style().visuals;
    let stroke = egui::Stroke::new(
        scale.max(0.5),
        preview_color(
            visuals.widgets.noninteractive.bg_stroke.color,
            context.opacity,
        ),
    );
    let header_fill = preview_color(visuals.faint_bg_color, context.opacity);
    let cell_fill = preview_color(visuals.extreme_bg_color, context.opacity);
    let text_color = preview_color(visuals.text_color(), context.opacity);
    let strong_text = preview_color(visuals.strong_text_color(), context.opacity);
    let font = egui::FontId::proportional((14.0 * scale).max(6.0));
    let small_font = egui::FontId::proportional((12.0 * scale).max(6.0));

    let mut x = rect.left();
    paint_preview_cell(
        context.painter,
        egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(row_header_width, row_height),
        ),
        header_fill,
        stroke,
        "",
        egui::Align2::LEFT_CENTER,
        strong_text,
        font.clone(),
        8.0 * scale,
    );
    x += row_header_width;
    for field in fields {
        let width = column_width(field.field_type) * scale;
        paint_preview_cell(
            context.painter,
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, row_height)),
            header_fill,
            stroke,
            &field.name,
            egui::Align2::LEFT_CENTER,
            strong_text,
            font.clone(),
            8.0 * scale,
        );
        x += width;
    }

    for (row_index, row) in rows.iter().enumerate() {
        let y = rect.top() + row_height * (row_index + 1) as f32;
        paint_preview_cell(
            context.painter,
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), y),
                egui::vec2(row_header_width, row_height),
            ),
            header_fill,
            stroke,
            &(row_index + 1).to_string(),
            egui::Align2::CENTER_CENTER,
            text_color,
            small_font.clone(),
            0.0,
        );
        let mut x = rect.left() + row_header_width;
        for field in fields {
            let width = column_width(field.field_type) * scale;
            let alignment = match field.field_type {
                DatabaseFieldType::String => egui::Align2::LEFT_CENTER,
                DatabaseFieldType::Number => egui::Align2::RIGHT_CENTER,
            };
            paint_preview_cell(
                context.painter,
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, row_height)),
                cell_fill,
                stroke,
                &cell_text(row, field),
                alignment,
                text_color,
                font.clone(),
                6.0 * scale,
            );
            x += width;
        }
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

fn take_typed_text(ui: &mut egui::Ui) -> Option<String> {
    ui.input_mut(|input| {
        let index = input.events.iter().position(|event| {
            matches!(event, egui::Event::Text(text) if !text.is_empty())
                || matches!(event, egui::Event::Paste(text) if !text.is_empty())
        })?;
        match input.events.remove(index) {
            egui::Event::Text(text) | egui::Event::Paste(text) => Some(text),
            _ => None,
        }
    })
}

fn cell_text(row: &DatabaseRow, field: &DatabaseField) -> String {
    match row.value(field.id) {
        Some(value) => database_value_text(value),
        None => String::new(),
    }
}

fn database_value_text(value: &DatabaseValue) -> String {
    match value {
        DatabaseValue::String(value) => value.clone(),
        DatabaseValue::Number(value) => value.to_string(),
    }
}

fn parse_cell_value(value: &str, field_type: DatabaseFieldType) -> Option<DatabaseValue> {
    match field_type {
        DatabaseFieldType::String => Some(DatabaseValue::String(value.to_owned())),
        DatabaseFieldType::Number => value.parse().ok().map(DatabaseValue::Number),
    }
}

fn column_width(field_type: DatabaseFieldType) -> f32 {
    match field_type {
        DatabaseFieldType::String => STRING_COLUMN_WIDTH,
        DatabaseFieldType::Number => NUMBER_COLUMN_WIDTH,
    }
}

fn grid_focus_id(database_id: Uuid) -> egui::Id {
    egui::Id::new(("database-grid-focus", database_id))
}

fn formula_input_id(database_id: Uuid) -> egui::Id {
    egui::Id::new(("database-formula-input", database_id))
}

fn column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            return name;
        }
        index = index / 26 - 1;
    }
}

fn field_type_label(field_type: DatabaseFieldType) -> &'static str {
    match field_type {
        DatabaseFieldType::String => "Text",
        DatabaseFieldType::Number => "Number",
    }
}
