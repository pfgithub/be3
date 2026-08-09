use block_client::{
    blocks::{
        database::{DatabaseOperation, DatabaseRow, DatabaseValue},
        database_schema::{DatabaseField, DatabaseFieldType},
        database_view::{DatabaseView, DatabaseViewOperation},
    },
    BlockHandle,
};
use eframe::egui;
use egui_material_icons::icons::ICON_ADD;
use uuid::Uuid;

use super::{cell_text, paint_preview_cell, preview_color, BlockRenderContext};

const COLUMN_WIDTH: f32 = 240.0;
const CARD_SPACING: f32 = 6.0;
const PREVIEW_COLUMN_GAP: f32 = 6.0;
const PREVIEW_CARD_HEIGHT: f32 = 16.0;
const PREVIEW_CARD_GAP: f32 = 4.0;

/// Selection and drag state for the kanban board. Cards are addressed by
/// their row's storage index, the same as spreadsheet cells.
#[derive(Default)]
pub(super) struct KanbanView {
    selected: Option<usize>,
    dragging: Option<usize>,
}

impl KanbanView {
    pub(super) fn selected_row(&self) -> Option<usize> {
        self.selected
    }

    pub(super) fn board(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        status_field: &DatabaseField,
        operations: &mut Vec<DatabaseOperation>,
    ) {
        egui::ScrollArea::horizontal()
            .id_salt(("database-kanban-scroll", view.id()))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    for option in &status_field.options {
                        self.column(
                            ui,
                            view,
                            rows,
                            fields,
                            status_field,
                            Some(option.id),
                            &option.name,
                            operations,
                        );
                    }
                    self.column(
                        ui,
                        view,
                        rows,
                        fields,
                        status_field,
                        None,
                        "No status",
                        operations,
                    );
                });
            });
        if ui.input(|input| input.pointer.any_released()) {
            self.dragging = None;
        }
    }

    fn column(
        &mut self,
        ui: &mut egui::Ui,
        view: &BlockHandle<DatabaseView>,
        rows: &[DatabaseRow],
        fields: &[DatabaseField],
        status_field: &DatabaseField,
        option_id: Option<Uuid>,
        title: &str,
        operations: &mut Vec<DatabaseOperation>,
    ) {
        let visuals = ui.visuals();
        let frame = egui::Frame::new()
            .fill(visuals.faint_bg_color)
            .stroke(visuals.widgets.noninteractive.bg_stroke)
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(COLUMN_WIDTH);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).strong());
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt((
                            "database-kanban-column",
                            view.id(),
                            status_field.id,
                            option_id,
                        ))
                        .max_height(480.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (row_index, row) in rows.iter().enumerate() {
                                let value =
                                    row.value(status_field.id).and_then(|value| match value {
                                        DatabaseValue::Enum(id) => Some(*id),
                                        _ => None,
                                    });
                                if value != option_id {
                                    continue;
                                }
                                self.card(ui, row_index, row, fields, status_field.id);
                            }
                        });
                    if ui
                        .button(format!("{} Add card", ICON_ADD.codepoint))
                        .clicked()
                    {
                        self.selected = Some(rows.len());
                        operations.push(DatabaseOperation::SetCell {
                            row_index: rows.len(),
                            field_id: status_field.id,
                            value: option_id.map(DatabaseValue::Enum),
                        });
                    }
                });
            });
        if self.dragging.is_some()
            && frame.response.hovered()
            && ui.input(|input| input.pointer.any_released())
        {
            let dragging = self.dragging.take().unwrap();
            operations.push(DatabaseOperation::SetCell {
                row_index: dragging,
                field_id: status_field.id,
                value: option_id.map(DatabaseValue::Enum),
            });
        }
    }

    fn card(
        &mut self,
        ui: &mut egui::Ui,
        row_index: usize,
        row: &DatabaseRow,
        fields: &[DatabaseField],
        status_field_id: Uuid,
    ) {
        let selected = self.selected == Some(row_index);
        let visuals = ui.visuals();
        let inner = egui::Frame::new()
            .fill(if selected {
                visuals.selection.bg_fill
            } else {
                visuals.extreme_bg_color
            })
            .stroke(egui::Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected {
                    visuals.selection.stroke.color
                } else {
                    visuals.widgets.noninteractive.bg_stroke.color
                },
            ))
            .corner_radius(4.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                ui.set_width(COLUMN_WIDTH - 16.0);
                let mut shown_any = false;
                for field in fields {
                    if field.id == status_field_id {
                        continue;
                    }
                    let text = cell_text(row, field);
                    if text.is_empty() {
                        continue;
                    }
                    shown_any = true;
                    ui.label(text);
                }
                if !shown_any {
                    ui.weak(format!("Row {}", row_index + 1));
                }
            });
        let response = inner.response.interact(egui::Sense::click_and_drag());
        if response.clicked() {
            self.selected = Some(row_index);
        }
        if response.drag_started() {
            self.dragging = Some(row_index);
        }
        ui.add_space(CARD_SPACING);
    }
}

/// Dropdown that chooses the enum field whose options become the board's
/// columns. Only enum fields have a fixed set of options, so only they can
/// stand in for a "status".
pub(super) fn status_field_picker(
    ui: &mut egui::Ui,
    view: &BlockHandle<DatabaseView>,
    fields: &[DatabaseField],
    current: Option<Uuid>,
) {
    let enum_fields: Vec<&DatabaseField> = fields
        .iter()
        .filter(|field| field.field_type == DatabaseFieldType::Enum)
        .collect();
    ui.horizontal(|ui| {
        ui.label("Status field:");
        let current_name = current
            .and_then(|id| fields.iter().find(|field| field.id == id))
            .map_or("Choose a field", |field| field.name.as_str());
        egui::ComboBox::new(("database-kanban-field", view.id()), "")
            .selected_text(current_name)
            .show_ui(ui, |ui| {
                for field in &enum_fields {
                    if ui
                        .selectable_label(current == Some(field.id), &field.name)
                        .clicked()
                    {
                        view.operate(DatabaseViewOperation::SetKanbanField {
                            field_id: Some(field.id),
                        });
                    }
                }
            });
        if enum_fields.is_empty() {
            ui.weak("Add an Enum column to use the kanban view.");
        }
    });
}

/// Column-and-card-count thumbnail, painted at the block's on-canvas size
/// rather than the interactive layout the direct editor uses.
pub(super) fn paint_preview(
    context: BlockRenderContext<'_>,
    rows: &[DatabaseRow],
    fields: &[DatabaseField],
    kanban_field_id: Option<Uuid>,
) {
    let rect = egui::Rect::from_min_max(context.corners[0], context.corners[2]);
    let Some(status_field) = kanban_field_id
        .and_then(|id| fields.iter().find(|field| field.id == id))
        .filter(|field| !field.options.is_empty())
    else {
        return;
    };
    let visuals = &context.painter.ctx().global_style().visuals;
    let stroke = egui::Stroke::new(
        1.0_f32,
        preview_color(
            visuals.widgets.noninteractive.bg_stroke.color,
            context.opacity,
        ),
    );
    let header_fill = preview_color(visuals.faint_bg_color, context.opacity);
    let card_fill = preview_color(visuals.extreme_bg_color, context.opacity);
    let strong_text = preview_color(visuals.strong_text_color(), context.opacity);

    let column_count = status_field.options.len();
    let column_width = (rect.width()
        - PREVIEW_COLUMN_GAP * (column_count.saturating_sub(1)) as f32)
        / column_count.max(1) as f32;
    let header_height = (rect.height() * 0.18).clamp(10.0, 22.0);
    let font = egui::FontId::proportional((11.0_f32).min(column_width * 0.3).max(4.0));

    for (index, option) in status_field.options.iter().enumerate() {
        let x = rect.left() + (column_width + PREVIEW_COLUMN_GAP) * index as f32;
        let column_rect = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(column_width, rect.height()),
        );
        let header_rect =
            egui::Rect::from_min_size(column_rect.min, egui::vec2(column_width, header_height));
        paint_preview_cell(
            context.painter,
            header_rect,
            header_fill,
            stroke,
            &option.name,
            egui::Align2::CENTER_CENTER,
            strong_text,
            font.clone(),
            2.0,
        );
        let mut y = header_rect.bottom() + PREVIEW_CARD_GAP;
        for row in rows {
            let matches = row
                .value(status_field.id)
                .is_some_and(|value| matches!(value, DatabaseValue::Enum(id) if *id == option.id));
            if !matches {
                continue;
            }
            if y + PREVIEW_CARD_HEIGHT > column_rect.bottom() {
                break;
            }
            let card_rect = egui::Rect::from_min_size(
                egui::pos2(column_rect.left() + 2.0, y),
                egui::vec2((column_width - 4.0).max(1.0), PREVIEW_CARD_HEIGHT),
            );
            context
                .painter
                .rect(card_rect, 2.0, card_fill, stroke, egui::StrokeKind::Inside);
            y += PREVIEW_CARD_HEIGHT + PREVIEW_CARD_GAP;
        }
    }
}
