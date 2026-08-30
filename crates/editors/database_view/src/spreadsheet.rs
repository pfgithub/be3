use block_client::{
    blocks::{
        database::{DatabaseOperation, DatabaseRow, DatabaseValue},
        database_schema::{DatabaseField, DatabaseFieldType},
        database_view::{DatabaseView, DatabaseViewOperation, DatabaseViewSort, SortDirection},
    },
    BlockHandle,
};
use block_editor_plugin::egui;
use block_editor_plugin::egui_material_icons::icons::{ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD};
use uuid::Uuid;

use crate::app::{
    cell_text, database_value_text, field_type_label, paint_preview_cell, parse_cell_value,
    preview_color, BlockRenderContext,
};

const ROW_HEADER_WIDTH: f32 = 44.0;
const ROW_HEIGHT: f32 = 28.0;
const STRING_COLUMN_WIDTH: f32 = 180.0;
const NUMBER_COLUMN_WIDTH: f32 = 120.0;

type DisplayRows = Vec<(usize, DatabaseRow)>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellAddress {
    row_index: usize,
    field_id: Uuid,
}

#[derive(Default)]
pub(crate) struct SpreadsheetView {
    selected: Option<CellAddress>,
    editing: Option<CellAddress>,
    edit_buffer: String,
    edit_original: Option<DatabaseValue>,
    request_edit_focus: bool,
}

impl SpreadsheetView {
    pub(crate) fn selected_row(&self) -> Option<usize> {
        self.selected.map(|address| address.row_index)
    }

    pub(crate) fn deselect(&mut self) {
        self.finish_edit();
        self.selected = None;
    }

    pub(crate) fn intrinsic_size(&self, row_count: usize, fields: &[DatabaseField]) -> egui::Vec2 {
        let total_rows = display_row_total(row_count, self.selected);
        egui::vec2(
            ROW_HEADER_WIDTH
                + fields
                    .iter()
                    .map(|field| column_width(field.field_type))
                    .sum::<f32>(),
            ROW_HEIGHT * (total_rows + 1) as f32,
        )
    }

    fn begin_edit(
        &mut self,
        row_index: usize,
        row: &DatabaseRow,
        field: &DatabaseField,
        replacement: Option<String>,
    ) {
        let address = CellAddress {
            row_index,
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

    fn select_cell(&mut self, row_index: usize, row: &DatabaseRow, field: &DatabaseField) {
        self.finish_edit();
        self.selected = Some(CellAddress {
            row_index,
            field_id: field.id,
        });
        self.edit_buffer = cell_text(row, field);
    }

    fn move_selection(
        &mut self,
        display: &DisplayRows,
        fields: &[DatabaseField],
        dx: isize,
        dy: isize,
    ) {
        let Some(selected) = self.selected else {
            if let (Some((row_index, row)), Some(field)) = (display.first(), fields.first()) {
                self.selected = Some(CellAddress {
                    row_index: *row_index,
                    field_id: field.id,
                });
                self.edit_buffer = cell_text(row, field);
            }
            return;
        };
        let Some(display_position) = display
            .iter()
            .position(|(row_index, _)| *row_index == selected.row_index)
        else {
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
        let display_position = display_position
            .saturating_add_signed(dy)
            .min(display.len() - 1);
        let field_index = field_index.saturating_add_signed(dx).min(fields.len() - 1);
        let (row_index, row) = &display[display_position];
        self.select_cell(*row_index, row, &fields[field_index]);
    }

    fn handle_keyboard(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        display: &DisplayRows,
        fields: &[DatabaseField],
        operations: &mut Vec<DatabaseOperation>,
    ) {
        if fields.is_empty() || self.editing.is_some() {
            return;
        }
        let focus_id = grid_focus_id(view.id());
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
            self.move_selection(display, fields, -1, 0);
        } else if ui
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
        {
            self.move_selection(display, fields, 1, 0);
        } else if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
        {
            self.move_selection(display, fields, 0, -1);
        } else if ui
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            self.move_selection(display, fields, 0, 1);
        } else {
            let backwards =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
            let forwards =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
            if backwards || forwards {
                self.move_selection(display, fields, if backwards { -1 } else { 1 }, 0);
            }
        }

        let Some(selected) = self.selected else {
            return;
        };
        let Some((_, row)) = display
            .iter()
            .find(|(row_index, _)| *row_index == selected.row_index)
        else {
            return;
        };
        let Some(field) = fields.iter().find(|field| field.id == selected.field_id) else {
            return;
        };

        if ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::F2)
        }) {
            self.begin_edit(selected.row_index, row, field, None);
        } else if ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
        }) {
            operations.push(DatabaseOperation::SetCell {
                row_index: selected.row_index,
                field_id: field.id,
                value: None,
            });
        } else if let Some(text) = take_typed_text(ui) {
            if let Some(value) = parse_cell_value(&text, field) {
                operations.push(DatabaseOperation::SetCell {
                    row_index: selected.row_index,
                    field_id: field.id,
                    value: Some(value),
                });
            }
            self.begin_edit(selected.row_index, row, field, Some(text));
        }
    }

    pub(crate) fn formula_bar(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        sort: Option<DatabaseViewSort>,
        operations: &mut Vec<DatabaseOperation>,
    ) {
        let display = display_rows(rows, self.selected, sort, fields);
        let selection = self.selected.and_then(|selected| {
            let display_position = display
                .iter()
                .position(|(row_index, _)| *row_index == selected.row_index)?;
            let field_index = fields
                .iter()
                .position(|field| field.id == selected.field_id)?;
            Some((selected, display_position, field_index))
        });
        let cell_label =
            selection.map_or_else(String::new, |(_, display_position, field_index)| {
                format!("{}{}", column_name(field_index), display_position + 1)
            });
        let formula_id = formula_input_id(view.id());
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

            let Some((address, display_position, field_index)) = selection else {
                return;
            };
            let (row_index, row) = &display[display_position];
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
                    parse_cell_value(&self.edit_buffer, field).map(Some)
                };
                if let Some(value) = value {
                    operations.push(DatabaseOperation::SetCell {
                        row_index: *row_index,
                        field_id: field.id,
                        value,
                    });
                }
            }
            if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Escape))
            {
                operations.push(DatabaseOperation::SetCell {
                    row_index: *row_index,
                    field_id: field.id,
                    value: self.edit_original.clone(),
                });
                self.edit_buffer = self
                    .edit_original
                    .as_ref()
                    .map_or_else(String::new, |value| database_value_text(value, field));
                self.finish_edit();
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(view.id())));
            } else if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                self.finish_edit();
                self.move_selection(&display, fields, 0, 1);
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(view.id())));
            } else if self.editing == Some(address)
                && ui.input(|input| input.key_pressed(egui::Key::Tab))
            {
                let backwards = ui.input(|input| input.modifiers.shift);
                self.finish_edit();
                self.move_selection(&display, fields, if backwards { -1 } else { 1 }, 0);
                ui.memory_mut(|memory| memory.request_focus(grid_focus_id(view.id())));
            }
        });
    }

    fn cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        row_index: usize,
        row: &DatabaseRow,
        field: &DatabaseField,
        scale: f32,
    ) {
        let address = CellAddress {
            row_index,
            field_id: field.id,
        };
        let selected = self.selected == Some(address);
        let size = egui::vec2(column_width(field.field_type), ROW_HEIGHT) * scale;
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        paint_cell_background(ui, rect, selected);
        paint_cell_text(ui, rect, &cell_text(row, field), field.field_type, scale);
        if response.clicked() {
            self.select_cell(row_index, row, field);
            ui.memory_mut(|memory| memory.request_focus(grid_focus_id(view.id())));
        }
        if response.double_clicked() {
            self.begin_edit(row_index, row, field, None);
        }
    }

    pub(crate) fn grid(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        sort: Option<DatabaseViewSort>,
        scale: f32,
        operations: &mut Vec<DatabaseOperation>,
    ) {
        let display = display_rows(rows, self.selected, sort, fields);
        self.handle_keyboard(ui, view, &display, fields, operations);
        let background = ui.interact(
            ui.available_rect_before_wrap(),
            ui.id().with(("database-grid-background", view.id())),
            egui::Sense::click(),
        );
        let mut sort_operation = None;
        egui::Grid::new(("database-grid", view.id()))
            .num_columns(fields.len() + 1)
            .spacing(egui::Vec2::ZERO)
            .show(ui, |ui| {
                spreadsheet_header(
                    ui,
                    egui::vec2(ROW_HEADER_WIDTH, ROW_HEIGHT) * scale,
                    "",
                    None,
                    scale,
                );
                for (column_index, field) in fields.iter().enumerate() {
                    let direction = sort
                        .filter(|sort| sort.field_id == field.id)
                        .map(|sort| sort.direction);
                    let response = spreadsheet_header(
                        ui,
                        egui::vec2(column_width(field.field_type), ROW_HEIGHT) * scale,
                        &field.name,
                        direction,
                        scale,
                    )
                    .on_hover_text(format!(
                        "Column {} · {}",
                        column_name(column_index),
                        field_type_label(field.field_type),
                    ));
                    if response.clicked() {
                        sort_operation = Some(DatabaseViewOperation::SetSort {
                            sort: next_sort(sort, field.id),
                        });
                    }
                }
                ui.end_row();

                for (display_position, (row_index, row)) in display.iter().enumerate() {
                    let selected = self
                        .selected
                        .is_some_and(|cell| cell.row_index == *row_index);
                    spreadsheet_row_header(ui, display_position + 1, selected, scale);
                    for field in fields {
                        self.cell_editor(ui, view, *row_index, row, field, scale);
                    }
                    ui.end_row();
                }
            });
        if background.clicked() {
            self.deselect();
        }
        if let Some(operation) = sort_operation {
            view.operate(operation);
        }
    }
}

fn next_sort(current: Option<DatabaseViewSort>, field_id: Uuid) -> Option<DatabaseViewSort> {
    match current {
        Some(sort) if sort.field_id == field_id => match sort.direction {
            SortDirection::Ascending => Some(DatabaseViewSort {
                field_id,
                direction: SortDirection::Descending,
            }),
            SortDirection::Descending => None,
        },
        _ => Some(DatabaseViewSort {
            field_id,
            direction: SortDirection::Ascending,
        }),
    }
}

fn compare_sorted_rows(
    a: &DatabaseRow,
    b: &DatabaseRow,
    sort: DatabaseViewSort,
    field: &DatabaseField,
) -> std::cmp::Ordering {
    let ordering = match (a.value(sort.field_id), b.value(sort.field_id)) {
        (Some(a), Some(b)) => compare_database_values(a, b, field),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    };
    match sort.direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
    }
}

pub(crate) fn sort_rows(
    rows: &mut [DatabaseRow],
    sort: DatabaseViewSort,
    fields: &[DatabaseField],
) {
    let Some(field) = fields.iter().find(|field| field.id == sort.field_id) else {
        return;
    };
    rows.sort_by(|a, b| compare_sorted_rows(a, b, sort, field));
}

fn sort_row_pairs(rows: &mut DisplayRows, sort: DatabaseViewSort, fields: &[DatabaseField]) {
    let Some(field) = fields.iter().find(|field| field.id == sort.field_id) else {
        return;
    };
    rows.sort_by(|(_, a), (_, b)| compare_sorted_rows(a, b, sort, field));
}

fn compare_database_values(
    a: &DatabaseValue,
    b: &DatabaseValue,
    field: &DatabaseField,
) -> std::cmp::Ordering {
    match (a, b) {
        (DatabaseValue::String(a), DatabaseValue::String(b)) => a.cmp(b),
        (DatabaseValue::Number(a), DatabaseValue::Number(b)) => {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (DatabaseValue::Enum(a), DatabaseValue::Enum(b)) => {
            let index = |id: Uuid| field.options.iter().position(|option| option.id == id);
            match (index(*a), index(*b)) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }
        _ => std::cmp::Ordering::Equal,
    }
}

fn extra_row_count(len: usize, selected: Option<CellAddress>) -> usize {
    let mut extra = 1;
    if let Some(selected) = selected {
        if selected.row_index >= len {
            extra = extra.max(selected.row_index - len + 2);
        }
    }
    extra
}

fn display_row_total(len: usize, selected: Option<CellAddress>) -> usize {
    len + extra_row_count(len, selected)
}

fn display_rows(
    rows: &[DatabaseRow],
    selected: Option<CellAddress>,
    sort: Option<DatabaseViewSort>,
    fields: &[DatabaseField],
) -> DisplayRows {
    let extra = extra_row_count(rows.len(), selected);
    let mut display: DisplayRows = rows.iter().cloned().enumerate().collect();
    if let Some(sort) = sort {
        sort_row_pairs(&mut display, sort, fields);
    }
    display.extend(
        std::iter::repeat_with(DatabaseRow::default)
            .take(extra)
            .enumerate()
            .map(|(i, row)| (rows.len() + i, row)),
    );
    display
}

fn spreadsheet_header(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    text: &str,
    sort: Option<SortDirection>,
    scale: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
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
    if let Some(direction) = sort {
        let icon = match direction {
            SortDirection::Ascending => ICON_ARROW_UPWARD,
            SortDirection::Descending => ICON_ARROW_DOWNWARD,
        };
        ui.painter().with_clip_rect(rect).text(
            rect.right_center() - egui::vec2(8.0 * scale, 0.0),
            egui::Align2::RIGHT_CENTER,
            icon.codepoint,
            egui::FontId::new((14.0 * scale).max(6.0), icon.font_family()),
            visuals.strong_text_color(),
        );
    }
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
            egui::Stroke::new(2.0_f32, visuals.selection.stroke.color),
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
        DatabaseFieldType::String | DatabaseFieldType::Enum => (
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

pub(crate) fn paint_preview(
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
                DatabaseFieldType::String | DatabaseFieldType::Enum => egui::Align2::LEFT_CENTER,
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

fn column_width(field_type: DatabaseFieldType) -> f32 {
    match field_type {
        DatabaseFieldType::String | DatabaseFieldType::Enum => STRING_COLUMN_WIDTH,
        DatabaseFieldType::Number => NUMBER_COLUMN_WIDTH,
    }
}

fn grid_focus_id(view_id: Uuid) -> egui::Id {
    egui::Id::new(("database-grid-focus", view_id))
}

fn formula_input_id(view_id: Uuid) -> egui::Id {
    egui::Id::new(("database-formula-input", view_id))
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
