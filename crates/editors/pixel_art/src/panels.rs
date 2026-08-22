use block_client::blocks::pixel_art::{
    PixelArtAnchor, PixelArtOperation, PixelColor, MAX_PIXEL_ART_PALETTE_COLORS, MAX_PIXEL_ART_SIZE,
};
use block_editor_plugin::{
    egui::{self, Vec2},
    egui_material_icons::{
        icons::{
            ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_DOWNWARD, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD,
            ICON_CIRCLE, ICON_COLORIZE, ICON_CROP_SQUARE, ICON_DELETE, ICON_DIAGONAL_LINE,
            ICON_DOWNLOAD, ICON_DRAW, ICON_FIND_REPLACE, ICON_FIT_SCREEN, ICON_FORMAT_COLOR_FILL,
            ICON_INK_ERASER, ICON_NORTH_EAST, ICON_NORTH_WEST, ICON_RESIZE, ICON_SOUTH_EAST,
            ICON_SOUTH_WEST, ICON_SQUARE, ICON_ZOOM_IN, ICON_ZOOM_OUT,
        },
        MaterialIcon,
    },
};

use crate::{
    app::PixelArtApp,
    canvas::{View, ZOOM_STEP},
    color::{color_swatch, parse_hex_color},
    drawing::{BrushShape, PixelTool, MAX_BRUSH_SIZE},
};

const NO_EDIT_ACCESS: &str = "You cannot change this block";

const fn tool_icon(tool: PixelTool) -> MaterialIcon {
    match tool {
        PixelTool::Pencil => ICON_DRAW,
        PixelTool::Eraser => ICON_INK_ERASER,
        PixelTool::Fill => ICON_FORMAT_COLOR_FILL,
        PixelTool::Eyedropper => ICON_COLORIZE,
        PixelTool::ReplaceColor => ICON_FIND_REPLACE,
        PixelTool::Line => ICON_DIAGONAL_LINE,
        PixelTool::Rectangle => ICON_CROP_SQUARE,
        PixelTool::Ellipse => ICON_CIRCLE,
    }
}

impl PixelArtApp {
    pub(crate) fn top_bar_ui(&mut self, ui: &mut egui::Ui, width: u16, height: u16) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(self.tool.label());
            ui.weak(format!("{width} × {height} px"));

            ui.separator();
            if ui
                .small_button(ICON_ZOOM_OUT)
                .on_hover_text("Zoom out (-)")
                .clicked()
            {
                self.view.change_zoom(1.0 / ZOOM_STEP, None);
            }
            if ui
                .button(format!("{:.0}%", self.view.zoom * 100.0))
                .on_hover_text("Reset zoom to 100%")
                .clicked()
            {
                self.view.change_zoom(1.0 / self.view.zoom, None);
            }
            if ui
                .small_button(ICON_ZOOM_IN)
                .on_hover_text("Zoom in (+)")
                .clicked()
            {
                self.view.change_zoom(ZOOM_STEP, None);
            }
            if ui
                .small_button(ICON_FIT_SCREEN)
                .on_hover_text("Fit canvas to viewport (0)")
                .clicked()
            {
                self.view = View::default();
            }
            ui.checkbox(&mut self.show_grid, "Grid");

            let editable = self.editable();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(editable, egui::Button::new(ICON_DOWNLOAD))
                    .on_hover_text("Export PNG")
                    .on_disabled_hover_text(NO_EDIT_ACCESS)
                    .clicked()
                {
                    self.export();
                }
                if ui
                    .add_enabled(editable, egui::Button::new(ICON_DELETE))
                    .on_hover_text("Clear")
                    .on_disabled_hover_text(NO_EDIT_ACCESS)
                    .clicked()
                {
                    self.active_drawing = None;
                    self.committed_preview = None;
                    self.clear_open = true;
                }
                if ui
                    .add_enabled(editable, egui::Button::new(ICON_RESIZE))
                    .on_hover_text("Resize")
                    .on_disabled_hover_text(NO_EDIT_ACCESS)
                    .clicked()
                {
                    self.active_drawing = None;
                    self.committed_preview = None;
                    self.resize_width = width;
                    self.resize_height = height;
                    self.resize_anchor = PixelArtAnchor::Center;
                    self.resize_open = true;
                }
            });
        });
        if let Some(error) = &self.export_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    pub(crate) fn tools_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tools");
        ui.add_space(4.0);
        egui::Grid::new("pixel-art-tools")
            .num_columns(2)
            .spacing(Vec2::new(4.0, 4.0))
            .show(ui, |ui| {
                self.tool_button(ui, PixelTool::Pencil, "B or P");
                self.tool_button(ui, PixelTool::Eraser, "E");
                ui.end_row();
                self.tool_button(ui, PixelTool::Fill, "G");
                self.tool_button(ui, PixelTool::Eyedropper, "I");
                ui.end_row();
                self.tool_button(ui, PixelTool::Line, "L");
                self.tool_button(ui, PixelTool::ReplaceColor, "C");
                ui.end_row();
                self.tool_button(ui, PixelTool::Rectangle, "R");
                self.tool_button(ui, PixelTool::Ellipse, "O");
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Tool options");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add(
                egui::DragValue::new(&mut self.brush_size)
                    .range(1..=MAX_BRUSH_SIZE)
                    .suffix(" px"),
            )
            .on_hover_text("Brush size ([ or ])");
        });
        ui.horizontal(|ui| {
            ui.label("Brush");
            ui.selectable_value(&mut self.brush_shape, BrushShape::Square, ICON_SQUARE)
                .on_hover_text("Square");
            ui.selectable_value(&mut self.brush_shape, BrushShape::Circle, ICON_CIRCLE)
                .on_hover_text("Circle");
        });
        let shapes_selected = matches!(self.tool, PixelTool::Rectangle | PixelTool::Ellipse);
        if ui
            .add_enabled(
                shapes_selected,
                egui::Button::new(ICON_FORMAT_COLOR_FILL).selected(self.shapes_filled),
            )
            .on_hover_text("Fill rectangles and ellipses (X)")
            .clicked()
        {
            self.shapes_filled = !self.shapes_filled;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Symmetry");
        ui.checkbox(&mut self.mirror_horizontal, "Mirror horizontally")
            .on_hover_text("Mirror across the vertical canvas axis (H)");
        ui.checkbox(&mut self.mirror_vertical, "Mirror vertically")
            .on_hover_text("Mirror across the horizontal canvas axis (V)");
    }

    pub(crate) fn colors_ui(&mut self, ui: &mut egui::Ui, palette: &[PixelColor]) {
        let editable = self.editable();
        ui.heading("Color");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut color = self.color.rgba();
            if ui
                .color_edit_button_srgba_unmultiplied(&mut color)
                .on_hover_text("Drawing color")
                .changed()
            {
                self.set_active_color(
                    PixelColor::new(color[0], color[1], color[2], color[3]),
                    false,
                );
            }
            let hex_response = ui.add(
                egui::TextEdit::singleline(&mut self.color_hex)
                    .desired_width(100.0)
                    .hint_text("#RRGGBBAA"),
            );
            if hex_response.changed() {
                if let Some(color) = parse_hex_color(&self.color_hex) {
                    self.color = color;
                }
            }
            if parse_hex_color(&self.color_hex).is_none() {
                hex_response.on_hover_text("Enter a color as #RRGGBBAA");
            }
        });

        let mut channels = self.color.rgba();
        let mut channels_changed = false;
        egui::Grid::new("pixel-art-channels")
            .num_columns(2)
            .show(ui, |ui| {
                for (label, channel) in ["Red", "Green", "Blue", "Alpha"]
                    .into_iter()
                    .zip(&mut channels)
                {
                    ui.label(label);
                    channels_changed |= ui
                        .add(egui::DragValue::new(channel).range(0..=255))
                        .changed();
                    ui.end_row();
                }
            });
        if channels_changed {
            self.set_active_color(
                PixelColor::new(channels[0], channels[1], channels[2], channels[3]),
                false,
            );
        }

        if self.tool == PixelTool::ReplaceColor {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Replace");
                if let Some(source) = self.replace_source_hover {
                    color_swatch(ui, source, false);
                } else {
                    ui.weak("hover canvas");
                }
                ui.label(ICON_ARROW_FORWARD);
                color_swatch(ui, self.color, false);
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Palette");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for &color in palette {
                if color_swatch(ui, color, color == self.color).clicked() {
                    self.set_active_color(color, true);
                }
            }
        });
        ui.horizontal(|ui| {
            let can_add = editable
                && palette.len() < MAX_PIXEL_ART_PALETTE_COLORS
                && !palette.contains(&self.color);
            if ui
                .add_enabled(can_add, egui::Button::new(ICON_ADD))
                .on_hover_text("Add the active color to this artwork's palette")
                .clicked()
            {
                let mut colors = palette.to_vec();
                colors.push(self.color);
                self.operate(PixelArtOperation::SetPalette { colors });
            }
            let can_remove = editable && palette.contains(&self.color);
            if ui
                .add_enabled(can_remove, egui::Button::new(ICON_DELETE))
                .on_hover_text("Remove the active color from this artwork's palette")
                .clicked()
            {
                let colors = palette
                    .iter()
                    .copied()
                    .filter(|color| *color != self.color)
                    .collect();
                self.operate(PixelArtOperation::SetPalette { colors });
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        ui.strong("Recent");
        ui.add_space(4.0);
        let recent_colors = self.recent_colors.clone();
        ui.horizontal_wrapped(|ui| {
            for color in recent_colors {
                if color_swatch(ui, color, color == self.color).clicked() {
                    self.set_active_color(color, true);
                }
            }
        });
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, tool: PixelTool, shortcut: &str) {
        if ui
            .add_sized(
                [76.0, 24.0],
                egui::Button::new(tool_icon(tool)).selected(self.tool == tool),
            )
            .on_hover_text(format!("{} ({shortcut})", tool.label()))
            .clicked()
        {
            self.select_tool(tool);
        }
    }

    /// The resize and clear confirmations, drawn inside the main region
    /// because that is the surface this editor is given.
    pub(crate) fn dialogs_ui(&mut self, ui: &mut egui::Ui, width: u16, height: u16) {
        if self.resize_open {
            let mut apply = false;
            let response =
                egui::Modal::new(egui::Id::new("pixel-art-resize")).show(ui.ctx(), |ui| {
                    ui.set_width(280.0);
                    ui.heading("Resize Pixel Art");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(&mut self.resize_width)
                                .range(1..=MAX_PIXEL_ART_SIZE),
                        );
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(&mut self.resize_height)
                                .range(1..=MAX_PIXEL_ART_SIZE),
                        );
                    });
                    ui.add_space(8.0);
                    ui.label("Anchor");
                    anchor_selector(ui, &mut self.resize_anchor);
                    if self.resize_width < width || self.resize_height < height {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            "Shrinking crops pixels outside the anchored region.",
                        );
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        apply = ui.button("Resize").clicked();
                        if ui.button("Cancel").clicked() {
                            self.resize_open = false;
                        }
                    });
                });
            if apply {
                self.operate(PixelArtOperation::Resize {
                    width: self.resize_width,
                    height: self.resize_height,
                    anchor: self.resize_anchor,
                });
                self.active_drawing = None;
                self.view = View::default();
                self.resize_open = false;
            } else if response.should_close() {
                self.resize_open = false;
            }
        }

        if self.clear_open {
            let mut clear = false;
            let response =
                egui::Modal::new(egui::Id::new("pixel-art-clear")).show(ui.ctx(), |ui| {
                    ui.set_width(280.0);
                    ui.heading("Clear Pixel Art?");
                    ui.add_space(8.0);
                    ui.label("This will make every pixel transparent.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        clear = ui.button("Clear").clicked();
                        if ui.button("Cancel").clicked() {
                            self.clear_open = false;
                        }
                    });
                });
            if clear {
                self.operate(PixelArtOperation::Clear);
                self.clear_open = false;
            } else if response.should_close() {
                self.clear_open = false;
            }
        }
    }
}

fn anchor_selector(ui: &mut egui::Ui, anchor: &mut PixelArtAnchor) {
    for row in [
        [
            (PixelArtAnchor::TopLeft, ICON_NORTH_WEST),
            (PixelArtAnchor::Top, ICON_ARROW_UPWARD),
            (PixelArtAnchor::TopRight, ICON_NORTH_EAST),
        ],
        [
            (PixelArtAnchor::Left, ICON_ARROW_BACK),
            (PixelArtAnchor::Center, ICON_CIRCLE),
            (PixelArtAnchor::Right, ICON_ARROW_FORWARD),
        ],
        [
            (PixelArtAnchor::BottomLeft, ICON_SOUTH_WEST),
            (PixelArtAnchor::Bottom, ICON_ARROW_DOWNWARD),
            (PixelArtAnchor::BottomRight, ICON_SOUTH_EAST),
        ],
    ] {
        ui.horizontal(|ui| {
            for (value, label) in row {
                ui.selectable_value(anchor, value, label);
            }
        });
    }
}
