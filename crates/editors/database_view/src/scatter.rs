use block_client::{
    blocks::{
        database::{DatabaseRow, DatabaseValue},
        database_schema::{DatabaseField, DatabaseFieldType},
        database_view::{DatabaseView, DatabaseViewOperation},
    },
    BlockHandle,
};
use block_editor_plugin::egui;
use uuid::Uuid;

use crate::app::{preview_color, BlockRenderContext};

const POINT_RADIUS: f32 = 4.0;
const SELECTED_POINT_RADIUS: f32 = 6.0;
const HIT_RADIUS: f32 = 10.0;
const AXIS_MARGIN_LEFT: f32 = 44.0;
const AXIS_MARGIN_BOTTOM: f32 = 28.0;
const AXIS_MARGIN_TOP: f32 = 8.0;
const AXIS_MARGIN_RIGHT: f32 = 8.0;

#[derive(Default)]
pub(crate) struct ScatterView {
    selected: Option<usize>,
}

impl ScatterView {
    pub(crate) fn selected_row(&self) -> Option<usize> {
        self.selected
    }

    pub(crate) fn deselect(&mut self) {
        self.selected = None;
    }

    pub(crate) fn plot(
        &mut self,
        ui: &mut egui::Ui,
        rows: &[DatabaseRow],
        x_field: &DatabaseField,
        y_field: &DatabaseField,
    ) {
        let rect = ui.available_rect_before_wrap();
        let visuals = ui.visuals().clone();
        ui.painter()
            .rect_filled(rect, 0.0, visuals.extreme_bg_color);

        let plot_rect = egui::Rect::from_min_max(
            rect.min + egui::vec2(AXIS_MARGIN_LEFT, AXIS_MARGIN_TOP),
            rect.max - egui::vec2(AXIS_MARGIN_RIGHT, AXIS_MARGIN_BOTTOM),
        );
        let points: Vec<(usize, f64, f64)> = rows
            .iter()
            .enumerate()
            .filter_map(|(row_index, row)| {
                Some((
                    row_index,
                    number_value(row, x_field.id)?,
                    number_value(row, y_field.id)?,
                ))
            })
            .collect();

        let response = ui.interact(
            rect,
            ui.id()
                .with(("database-scatter-plot", x_field.id, y_field.id)),
            egui::Sense::click(),
        );

        if points.is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No rows have both fields set",
                egui::FontId::proportional(14.0),
                visuals.weak_text_color(),
            );
            return;
        }

        let (x_min, x_max) = bounds(points.iter().map(|(_, x, _)| *x));
        let (y_min, y_max) = bounds(points.iter().map(|(_, _, y)| *y));
        let to_screen = |x: f64, y: f64| -> egui::Pos2 {
            egui::pos2(
                plot_rect.left() + normalize(x, x_min, x_max) as f32 * plot_rect.width(),
                plot_rect.bottom() - normalize(y, y_min, y_max) as f32 * plot_rect.height(),
            )
        };

        if response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.selected = points
                    .iter()
                    .map(|(row_index, x, y)| (*row_index, to_screen(*x, *y).distance(pointer)))
                    .filter(|(_, distance)| *distance <= HIT_RADIUS)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(row_index, _)| row_index);
            }
        }

        let axis_stroke = visuals.widgets.noninteractive.bg_stroke;
        let painter = ui.painter();
        painter.line_segment([plot_rect.left_top(), plot_rect.left_bottom()], axis_stroke);
        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_bottom()],
            axis_stroke,
        );

        for (row_index, x, y) in &points {
            let pos = to_screen(*x, *y);
            let selected = self.selected == Some(*row_index);
            let color = if selected {
                visuals.selection.bg_fill
            } else {
                visuals.strong_text_color()
            };
            painter.circle_filled(pos, POINT_RADIUS, color);
            if selected {
                painter.circle_stroke(
                    pos,
                    SELECTED_POINT_RADIUS,
                    egui::Stroke::new(1.5_f32, visuals.selection.stroke.color),
                );
            }
        }

        let label_font = egui::FontId::proportional(11.0);
        let text_color = visuals.text_color();
        painter.text(
            plot_rect.left_bottom() + egui::vec2(2.0, 2.0),
            egui::Align2::LEFT_TOP,
            format_axis_value(x_min),
            label_font.clone(),
            text_color,
        );
        painter.text(
            plot_rect.right_bottom() + egui::vec2(0.0, 2.0),
            egui::Align2::RIGHT_TOP,
            format_axis_value(x_max),
            label_font.clone(),
            text_color,
        );
        painter.text(
            plot_rect.center_bottom() + egui::vec2(0.0, 14.0),
            egui::Align2::CENTER_TOP,
            &x_field.name,
            label_font.clone(),
            visuals.strong_text_color(),
        );
        painter.text(
            plot_rect.left_bottom() + egui::vec2(-4.0, 0.0),
            egui::Align2::RIGHT_BOTTOM,
            format_axis_value(y_min),
            label_font.clone(),
            text_color,
        );
        painter.text(
            plot_rect.left_top() + egui::vec2(-4.0, 0.0),
            egui::Align2::RIGHT_TOP,
            format_axis_value(y_max),
            label_font.clone(),
            text_color,
        );
        painter.text(
            plot_rect.left_center() - egui::vec2(AXIS_MARGIN_LEFT - 12.0, 0.0),
            egui::Align2::CENTER_CENTER,
            &y_field.name,
            label_font,
            visuals.strong_text_color(),
        );
    }
}

fn number_value(row: &DatabaseRow, field_id: Uuid) -> Option<f64> {
    match row.value(field_id) {
        Some(DatabaseValue::Number(value)) => Some(*value),
        _ => None,
    }
}

fn bounds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in values {
        min = min.min(value);
        max = max.max(value);
    }
    if min >= max {
        (min - 1.0, max + 1.0)
    } else {
        (min, max)
    }
}

fn normalize(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        0.5
    } else {
        (value - min) / (max - min)
    }
}

fn format_axis_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub(crate) fn axis_field_pickers(
    ui: &mut egui::Ui,
    view: &BlockHandle<DatabaseView>,
    fields: &[DatabaseField],
    x_field_id: Option<Uuid>,
    y_field_id: Option<Uuid>,
) {
    let number_fields: Vec<&DatabaseField> = fields
        .iter()
        .filter(|field| field.field_type == DatabaseFieldType::Number)
        .collect();
    ui.horizontal(|ui| {
        ui.label("X:");
        axis_field_picker(
            ui,
            view,
            &number_fields,
            x_field_id,
            "database-scatter-x-field",
        );
        ui.label("Y:");
        axis_field_picker(
            ui,
            view,
            &number_fields,
            y_field_id,
            "database-scatter-y-field",
        );
        if number_fields.is_empty() {
            ui.weak("Add a Number column to use the scatter view.");
        }
    });
}

fn axis_field_picker(
    ui: &mut egui::Ui,
    view: &BlockHandle<DatabaseView>,
    fields: &[&DatabaseField],
    current: Option<Uuid>,
    salt: &'static str,
) {
    let current_name = current
        .and_then(|id| fields.iter().find(|field| field.id == id))
        .map_or("Choose a field", |field| field.name.as_str());
    egui::ComboBox::new((salt, view.id()), "")
        .selected_text(current_name)
        .show_ui(ui, |ui| {
            for field in fields {
                if ui
                    .selectable_label(current == Some(field.id), &field.name)
                    .clicked()
                {
                    let field_id = Some(field.id);
                    view.operate(if salt == "database-scatter-x-field" {
                        DatabaseViewOperation::SetScatterXField { field_id }
                    } else {
                        DatabaseViewOperation::SetScatterYField { field_id }
                    });
                }
            }
        });
}

pub(crate) fn paint_preview(
    context: BlockRenderContext<'_>,
    rows: &[DatabaseRow],
    fields: &[DatabaseField],
    x_field_id: Option<Uuid>,
    y_field_id: Option<Uuid>,
) {
    let Some(x_field) = x_field_id.and_then(|id| fields.iter().find(|field| field.id == id)) else {
        return;
    };
    let Some(y_field) = y_field_id.and_then(|id| fields.iter().find(|field| field.id == id)) else {
        return;
    };
    let points: Vec<(f64, f64)> = rows
        .iter()
        .filter_map(|row| {
            Some((
                number_value(row, x_field.id)?,
                number_value(row, y_field.id)?,
            ))
        })
        .collect();
    if points.is_empty() {
        return;
    }
    let rect = egui::Rect::from_min_max(context.corners[0], context.corners[2]).shrink(4.0);
    let (x_min, x_max) = bounds(points.iter().map(|(x, _)| *x));
    let (y_min, y_max) = bounds(points.iter().map(|(_, y)| *y));
    let visuals = &context.painter.ctx().global_style().visuals;
    let dot_color = preview_color(visuals.strong_text_color(), context.opacity);
    let radius = (rect.width().min(rect.height()) * 0.02).clamp(1.5, 4.0);
    for (x, y) in points {
        let pos = egui::pos2(
            rect.left() + normalize(x, x_min, x_max) as f32 * rect.width(),
            rect.bottom() - normalize(y, y_min, y_max) as f32 * rect.height(),
        );
        context.painter.circle_filled(pos, radius, dot_color);
    }
}
