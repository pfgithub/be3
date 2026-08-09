use std::ops::RangeInclusive;

use block_client::blocks::map::MapColor;

use super::*;
use crate::editors::{paint_name, BlockLabel};

const COLOR_PRESETS: [(&str, MapColor); 5] = [
    ("Default", MapColor::Default),
    (
        "Orange",
        MapColor::Rgb {
            red: 240,
            green: 140,
            blue: 0,
        },
    ),
    (
        "Green",
        MapColor::Rgb {
            red: 47,
            green: 158,
            blue: 68,
        },
    ),
    (
        "Blue",
        MapColor::Rgb {
            red: 25,
            green: 113,
            blue: 194,
        },
    ),
    (
        "Violet",
        MapColor::Rgb {
            red: 121,
            green: 80,
            blue: 242,
        },
    ),
];

const PREVIEW_HEIGHT: f32 = 130.0;

impl MapEditor {
    pub(super) fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        ui.heading("Map");
        ui.separator();
        self.show_region_inspector(ui);
        ui.separator();
        let action = self.show_points(ui, editors);
        if let Some(error) = self.import_error.clone() {
            ui.separator();
            ui.colored_label(ui.visuals().error_fg_color, error);
            if ui.small_button("Dismiss").clicked() {
                self.import_error = None;
            }
        }
        action
    }

    fn show_region_inspector(&mut self, ui: &mut egui::Ui) {
        let region = self.preview_region();
        egui::CollapsingHeader::new("Preview region")
            .default_open(true)
            .show(ui, |ui| {
                let mut enabled = region.is_some();
                if ui
                    .checkbox(&mut enabled, "Show a fixed region")
                    .on_hover_text(
                        "Tabs and slides open zoomed to this region, and canvases clip to it.",
                    )
                    .changed()
                {
                    let region = enabled.then_some(self.visible_region);
                    self.record(MapOperation::SetPreviewRegion { region });
                }
                let Some(mut region) = region else {
                    ui.weak("Previews show the whole world.");
                    return;
                };
                let mut changed = false;
                egui::Grid::new("map-preview-region-fields")
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.label("North");
                        changed |= latitude_drag(ui, &mut region.north);
                        ui.label("South");
                        changed |= latitude_drag(ui, &mut region.south);
                        ui.end_row();
                        ui.label("West");
                        changed |= longitude_drag(ui, &mut region.west);
                        ui.label("East");
                        changed |= longitude_drag(ui, &mut region.east);
                        ui.end_row();
                    });
                if changed {
                    self.record_grouped(MapOperation::SetPreviewRegion {
                        region: Some(region),
                    });
                }
                ui.horizontal(|ui| {
                    if ui
                        .button("Use current view")
                        .on_hover_text("Capture the area the map is showing")
                        .clicked()
                    {
                        self.record(MapOperation::SetPreviewRegion {
                            region: Some(self.visible_region),
                        });
                    }
                    if ui.button("Zoom to region").clicked() {
                        self.fit_region_requested = true;
                    }
                });
            });
    }

    fn show_points(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let points = self.points();
        let labels = self.dependency_labels(editors);
        self.ensure_point_editors(&points, editors);
        let selected = self
            .selected
            .and_then(|id| points.iter().copied().find(|point| point.id == id));
        let Some(point) = selected else {
            return self.show_point_list(ui, &points, &labels);
        };
        self.show_point_details(ui, editors, point, labels.get(&point.block_id))
    }

    fn show_point_list(
        &mut self,
        ui: &mut egui::Ui,
        points: &[MapPoint],
        labels: &HashMap<Uuid, BlockLabel>,
    ) -> Option<EditorAction> {
        ui.strong("Points of interest");
        if points.is_empty() {
            ui.add_space(4.0);
            ui.weak(
                "Add blocks with the + button, by dragging them in from Files, or by pasting an image.",
            );
            return None;
        }
        ui.weak("Select a marker to edit it.");
        ui.add_space(4.0);
        let mut selected = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for point in points {
                let label = labels.get(&point.block_id).map_or_else(
                    || egui::RichText::new("Loading…").into(),
                    |label| label.widget_text(ui.style()),
                );
                if ui
                    .add(
                        egui::Button::new(label)
                            .right_text(
                                egui_material_icons::icons::ICON_CIRCLE
                                    .rich_text()
                                    .color(points::marker_color(point.color)),
                            )
                            .truncate(),
                    )
                    .clicked()
                {
                    selected = Some(*point);
                }
            }
        });
        if let Some(point) = selected {
            self.selected = Some(point.id);
            self.center_requested = Some(point.position);
        }
        None
    }

    fn show_point_details(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        point: MapPoint,
        label: Option<&BlockLabel>,
    ) -> Option<EditorAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .button(ICON_ARROW_BACK)
                .on_hover_text("Back to all points")
                .clicked()
            {
                self.selected = None;
            }
            let name = label.map_or_else(
                || egui::RichText::new("Loading…").strong().into(),
                |label| label.styled_widget_text(ui.style(), egui::RichText::strong),
            );
            ui.add(egui::Label::new(name).truncate());
        });
        ui.add_space(6.0);
        show_block_preview(ui, editors, point.block_id, label);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(label.is_some(), egui::Button::new("Edit"))
                .on_disabled_hover_text("Waiting for block metadata")
                .clicked()
            {
                if let Some(label) = label {
                    action = Some(EditorAction::OpenBlock {
                        id: point.block_id,
                        block_type: label.block_type,
                    });
                }
            }
            if ui
                .button(ICON_MY_LOCATION)
                .on_hover_text("Centre the map here")
                .clicked()
            {
                self.center_requested = Some(point.position);
            }
            if ui
                .button(ICON_DELETE)
                .on_hover_text("Remove this point of interest")
                .clicked()
            {
                self.remove_point(point.id);
            }
        });

        ui.add_space(8.0);
        ui.strong("Position");
        let mut updated = point;
        let mut changed = false;
        egui::Grid::new("map-point-position")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Latitude");
                changed |= latitude_drag(ui, &mut updated.position.latitude);
                ui.end_row();
                ui.label("Longitude");
                changed |= longitude_drag(ui, &mut updated.position.longitude);
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.strong("Colour");
        ui.horizontal(|ui| {
            for (name, color) in COLOR_PRESETS {
                if ui
                    .selectable_label(
                        point.color == color,
                        egui_material_icons::icons::ICON_CIRCLE
                            .rich_text()
                            .color(points::marker_color(color)),
                    )
                    .on_hover_text(name)
                    .clicked()
                {
                    updated.color = color;
                    changed = true;
                }
            }
            let current = points::marker_color(point.color).to_array();
            let mut rgb = [current[0], current[1], current[2]];
            if ui
                .color_edit_button_srgb(&mut rgb)
                .on_hover_text("Custom colour")
                .changed()
            {
                updated.color = MapColor::Rgb {
                    red: rgb[0],
                    green: rgb[1],
                    blue: rgb[2],
                };
                changed = true;
            }
        });
        if changed {
            self.record_grouped(MapOperation::UpdatePoints {
                points: vec![updated],
            });
        }
        action
    }
}

fn show_block_preview(
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    label: Option<&BlockLabel>,
) {
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), PREVIEW_HEIGHT),
        Sense::hover(),
    );
    let frame = response.rect;
    painter.rect_filled(frame, 4.0, ui.visuals().extreme_bg_color);
    let aspect = editors.preview_aspect_ratio(block_id).unwrap_or(1.0);
    let inner = fit_aspect_ratio(frame.shrink(4.0), aspect);
    let rendered = editors.render(
        block_id,
        BlockRenderContext {
            painter: &painter,
            corners: [
                inner.left_top(),
                inner.right_top(),
                inner.right_bottom(),
                inner.left_bottom(),
            ],
            opacity: 1.0,
        },
    );
    if !rendered {
        let (text, automatic) = label.map_or_else(
            || ("Loading…".to_owned(), false),
            |label| {
                (
                    editors.registry().icon_label(label.block_type, &label.name),
                    label.automatic,
                )
            },
        );
        paint_name(
            &painter,
            frame.center(),
            egui::Align2::CENTER_CENTER,
            &text,
            FontId::proportional(13.0),
            ui.visuals().weak_text_color(),
            automatic,
        );
    }
    painter.rect_stroke(
        frame,
        4.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
}

fn fit_aspect_ratio(rect: Rect, aspect: f32) -> Rect {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let size = if rect.width() / rect.height() > aspect {
        Vec2::new(rect.height() * aspect, rect.height())
    } else {
        Vec2::new(rect.width(), rect.width() / aspect)
    };
    Rect::from_center_size(rect.center(), size)
}

fn latitude_drag(ui: &mut egui::Ui, value: &mut f64) -> bool {
    coordinate_drag(ui, value, -MAX_LATITUDE..=MAX_LATITUDE)
}

fn longitude_drag(ui: &mut egui::Ui, value: &mut f64) -> bool {
    coordinate_drag(ui, value, -180.0..=180.0)
}

fn coordinate_drag(ui: &mut egui::Ui, value: &mut f64, range: RangeInclusive<f64>) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.01)
            .range(range)
            .max_decimals(6)
            .suffix("°"),
    )
    .changed()
}
