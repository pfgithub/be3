use std::collections::BTreeSet;

pub(super) mod dynamic_artifact;

use block::{Block, BlockParent};
use block_client::{
    blocks::image::Image,
    blocks::pixel_art::{
        PixelArt, PixelArtAnchor, PixelArtOperation, PixelColor, PixelUpdate,
        MAX_PIXEL_ART_PALETTE_COLORS, MAX_PIXEL_ART_SIZE,
    },
    BlockClient, BlockHandle, BlockRelationships,
};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_DOWNWARD, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD,
    ICON_CIRCLE, ICON_COLORIZE, ICON_CROP_SQUARE, ICON_DELETE, ICON_DIAGONAL_LINE, ICON_DOWNLOAD,
    ICON_DRAW, ICON_FIND_REPLACE, ICON_FIT_SCREEN, ICON_FORMAT_COLOR_FILL, ICON_GRID_ON,
    ICON_INK_ERASER, ICON_NORTH_EAST, ICON_NORTH_WEST, ICON_RESIZE, ICON_SOUTH_EAST,
    ICON_SOUTH_WEST, ICON_SQUARE, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorViewport, EditorAction,
    EditorRegistration,
};

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: PixelArt::TYPE_ID,
        display_name: "Pixel Art",
        icon: ICON_GRID_ON,
        create: Some(|client| Box::new(PixelArtEditor::new(client.create_block(PixelArt::new())))),
        open: |client, id| Box::new(PixelArtEditor::new(client.get_block::<PixelArt>(id))),
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: Some(dynamic_artifact::regenerate),
    }
}

const ZOOM_STEP: f32 = 1.25;
const MAX_BRUSH_SIZE: u16 = 64;
const MAX_RECENT_COLORS: usize = 12;
const DIRECT_EDITOR_LONG_SIDE: f32 = 256.0;
const DIRECT_EDITOR_SHORT_SIDE: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    ReplaceColor,
    Line,
    Rectangle,
    Ellipse,
}

impl PixelTool {
    const fn is_drawing(self) -> bool {
        matches!(
            self,
            Self::Pencil | Self::Eraser | Self::Line | Self::Rectangle | Self::Ellipse
        )
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Eyedropper => "Eyedropper",
            Self::ReplaceColor => "Replace Color",
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
        }
    }

    const fn icon(self) -> MaterialIcon {
        match self {
            Self::Pencil => ICON_DRAW,
            Self::Eraser => ICON_INK_ERASER,
            Self::Fill => ICON_FORMAT_COLOR_FILL,
            Self::Eyedropper => ICON_COLORIZE,
            Self::ReplaceColor => ICON_FIND_REPLACE,
            Self::Line => ICON_DIAGONAL_LINE,
            Self::Rectangle => ICON_CROP_SQUARE,
            Self::Ellipse => ICON_CIRCLE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrushShape {
    Square,
    Circle,
}

#[derive(Clone, Debug)]
struct ActiveDrawing {
    tool: PixelTool,
    start: (u16, u16),
    end: (u16, u16),
    path: Vec<(u16, u16)>,
}

#[derive(Clone, Debug)]
struct CommittedPreview {
    pixels: Vec<(u16, u16)>,
    color: PixelColor,
    frames_remaining: u8,
}

pub(super) struct PixelArtEditor {
    block: BlockHandle<PixelArt>,
    tool: PixelTool,
    previous_drawing_tool: PixelTool,
    color: PixelColor,
    color_hex: String,
    recent_colors: Vec<PixelColor>,
    replace_source_hover: Option<PixelColor>,
    brush_size: u16,
    brush_shape: BrushShape,
    shapes_filled: bool,
    mirror_horizontal: bool,
    mirror_vertical: bool,
    show_grid: bool,
    active_drawing: Option<ActiveDrawing>,
    committed_preview: Option<CommittedPreview>,
    texture: Option<TextureHandle>,
    edit_texture: Option<TextureHandle>,
    texture_image: Option<egui::ColorImage>,
    texture_preview: Vec<(u16, u16)>,
    texture_preview_color: Option<PixelColor>,
    texture_revision: Option<u64>,
    texture_size: [u16; 2],
    texture_dark_mode: bool,
    resize_open: bool,
    resize_width: u16,
    resize_height: u16,
    resize_anchor: PixelArtAnchor,
    clear_open: bool,
    export_error: Option<String>,
}

impl PixelArtEditor {
    pub(super) fn new(block: BlockHandle<PixelArt>) -> Self {
        Self {
            block,
            tool: PixelTool::Pencil,
            previous_drawing_tool: PixelTool::Pencil,
            color: PixelColor::new(0, 0, 0, 255),
            color_hex: "#000000FF".into(),
            recent_colors: vec![PixelColor::new(0, 0, 0, 255)],
            replace_source_hover: None,
            brush_size: 1,
            brush_shape: BrushShape::Square,
            shapes_filled: false,
            mirror_horizontal: false,
            mirror_vertical: false,
            show_grid: true,
            active_drawing: None,
            committed_preview: None,
            texture: None,
            edit_texture: None,
            texture_image: None,
            texture_preview: Vec::new(),
            texture_preview_color: None,
            texture_revision: None,
            texture_size: [0, 0],
            texture_dark_mode: false,
            resize_open: false,
            resize_width: 32,
            resize_height: 32,
            resize_anchor: PixelArtAnchor::Center,
            clear_open: false,
            export_error: None,
        }
    }

    fn record_pixels(&mut self, operation: PixelArtOperation) {
        self.block.operate(operation);
    }

    fn record_resize(&mut self, width: u16, height: u16, anchor: PixelArtAnchor) {
        self.block.operate(PixelArtOperation::Resize {
            width,
            height,
            anchor,
        });
    }

    fn top_bar(
        &mut self,
        ui: &mut egui::Ui,
        width: u16,
        height: u16,
        viewport: &mut DirectEditorViewport,
    ) -> bool {
        let mut export = false;
        ui.horizontal_wrapped(|ui| {
            ui.strong(self.tool.label());
            ui.weak(format!("{width} × {height} px"));

            ui.separator();
            if ui
                .small_button(ICON_ZOOM_OUT)
                .on_hover_text("Zoom out (-)")
                .clicked()
            {
                viewport.change_zoom(1.0 / ZOOM_STEP, None);
            }
            if ui
                .button(format!("{:.0}%", viewport.zoom() * 100.0))
                .on_hover_text("Reset zoom to 100%")
                .clicked()
            {
                viewport.change_zoom(1.0 / viewport.zoom(), None);
            }
            if ui
                .small_button(ICON_ZOOM_IN)
                .on_hover_text("Zoom in (+)")
                .clicked()
            {
                viewport.change_zoom(ZOOM_STEP, None);
            }
            if ui
                .small_button(ICON_FIT_SCREEN)
                .on_hover_text("Fit canvas to viewport (0)")
                .clicked()
            {
                viewport.fit();
            }
            ui.checkbox(&mut self.show_grid, "Grid");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                export = ui
                    .button(ICON_DOWNLOAD)
                    .on_hover_text("Export PNG")
                    .clicked();
                if ui.button(ICON_DELETE).on_hover_text("Clear").clicked() {
                    self.active_drawing = None;
                    self.committed_preview = None;
                    self.clear_open = true;
                }
                if ui.button(ICON_RESIZE).on_hover_text("Resize").clicked() {
                    self.active_drawing = None;
                    self.committed_preview = None;
                    self.resize_width = width;
                    self.resize_height = height;
                    self.resize_anchor = PixelArtAnchor::Center;
                    self.resize_open = true;
                }
            });
        });
        export
    }

    fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tools");
        ui.add_space(4.0);
        egui::Grid::new(("pixel-art-tools", self.block.id()))
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

    fn color_panel(&mut self, ui: &mut egui::Ui, palette: &[PixelColor]) {
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
        egui::Grid::new(("pixel-art-channels", self.block.id()))
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
            let can_add =
                palette.len() < MAX_PIXEL_ART_PALETTE_COLORS && !palette.contains(&self.color);
            if ui
                .add_enabled(can_add, egui::Button::new(ICON_ADD))
                .on_hover_text("Add the active color to this artwork's palette")
                .clicked()
            {
                let mut colors = palette.to_vec();
                colors.push(self.color);
                self.block.operate(PixelArtOperation::SetPalette { colors });
            }
            let can_remove = palette.contains(&self.color);
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
                self.block.operate(PixelArtOperation::SetPalette { colors });
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
                egui::Button::new(tool.icon()).selected(self.tool == tool),
            )
            .on_hover_text(format!("{} ({shortcut})", tool.label()))
            .clicked()
        {
            self.select_tool(tool);
        }
    }

    fn select_tool(&mut self, tool: PixelTool) {
        self.active_drawing = None;
        self.committed_preview = None;
        self.replace_source_hover = None;
        if self.tool.is_drawing() {
            self.previous_drawing_tool = self.tool;
        }
        if tool.is_drawing() {
            self.previous_drawing_tool = tool;
        }
        self.tool = tool;
    }

    fn remember_color(&mut self, color: PixelColor) {
        self.recent_colors.retain(|recent| *recent != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(MAX_RECENT_COLORS);
    }

    fn set_active_color(&mut self, color: PixelColor, remember: bool) {
        self.color = color;
        self.color_hex = format_hex_color(color);
        if remember {
            self.remember_color(color);
        }
    }

    fn update_texture(
        &mut self,
        context: &egui::Context,
        image: egui::ColorImage,
        revision: u64,
        size: [u16; 2],
        dark_mode: bool,
    ) {
        let edit_image = image.clone();
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(context.load_texture(
                format!("pixel-art-{}", self.block.id()),
                image,
                egui::TextureOptions::NEAREST,
            ));
        }
        if let Some(texture) = &mut self.edit_texture {
            texture.set(edit_image.clone(), egui::TextureOptions::NEAREST);
        } else {
            self.edit_texture = Some(context.load_texture(
                format!("pixel-art-edit-{}", self.block.id()),
                edit_image.clone(),
                egui::TextureOptions::NEAREST,
            ));
        }
        self.texture_image = Some(edit_image);
        self.texture_preview.clear();
        self.texture_preview_color = None;
        self.texture_revision = Some(revision);
        self.texture_size = size;
        self.texture_dark_mode = dark_mode;
    }

    fn ensure_texture(&mut self, context: &egui::Context, dark_mode: bool) -> Option<(u16, u16)> {
        let art = self.block.read()?;
        let width = art.width();
        let height = art.height();
        let revision = art.revision();
        let size = [width, height];
        let image = (self.texture_revision != Some(revision)
            || self.texture_size != size
            || self.texture_dark_mode != dark_mode)
            .then(|| checkerboard_image(&art, dark_mode));
        drop(art);
        if let Some(image) = image {
            self.update_texture(context, image, revision, size, dark_mode);
        }
        Some((width, height))
    }

    fn update_texture_preview(&mut self, pixels: &[(u16, u16)], color: PixelColor) {
        if self.texture_preview == pixels
            && (pixels.is_empty() || self.texture_preview_color == Some(color))
        {
            return;
        }
        let (Some(texture), Some(image)) = (&mut self.edit_texture, &self.texture_image) else {
            return;
        };

        let mut changed = BTreeSet::new();
        changed.extend(self.texture_preview.iter().map(|&(x, y)| (y, x)));
        changed.extend(pixels.iter().map(|&(x, y)| (y, x)));
        let mut changed = changed.into_iter().peekable();
        while let Some((y, start_x)) = changed.next() {
            let mut end_x = start_x;
            while changed
                .peek()
                .is_some_and(|&(next_y, x)| next_y == y && x == end_x + 1)
            {
                end_x = changed.next().expect("peeked changed pixel").1;
            }

            let row = (start_x..=end_x)
                .map(|x| {
                    if pixels.binary_search(&(x, y)).is_ok() {
                        let (light, dark) = checkerboard_colors(self.texture_dark_mode);
                        let background = if (x + y) % 2 == 0 { light } else { dark };
                        composite_pixel(color, background)
                    } else {
                        image[(usize::from(x), usize::from(y))]
                    }
                })
                .collect();
            texture.set_partial(
                [usize::from(start_x), usize::from(y)],
                egui::ColorImage::new([usize::from(end_x - start_x + 1), 1], row),
                egui::TextureOptions::NEAREST,
            );
        }
        self.texture_preview = pixels.to_vec();
        self.texture_preview_color = (!pixels.is_empty()).then_some(color);
    }

    fn paint_edit_texture(&self, painter: &egui::Painter, canvas: Rect) {
        if let Some(texture) = &self.edit_texture {
            painter.image(
                texture.id(),
                canvas,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    fn paint_texture(&self, context: BlockRenderContext<'_>) {
        let Some(texture) = &self.texture else {
            return;
        };
        let tint =
            Color32::from_white_alpha((context.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut mesh = egui::Mesh::with_texture(texture.id());
        mesh.vertices.extend([
            egui::epaint::Vertex {
                pos: context.corners[0],
                uv: Pos2::new(0.0, 0.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: context.corners[1],
                uv: Pos2::new(1.0, 0.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: context.corners[2],
                uv: Pos2::new(1.0, 1.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: context.corners[3],
                uv: Pos2::new(0.0, 1.0),
                color: tint,
            },
        ]);
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        context.painter.add(mesh);
    }

    fn export(&mut self, client: &BlockClient) -> Option<EditorAction> {
        let generated = self
            .block
            .read()
            .map(|art| dynamic_artifact::generate(&art, &self.block.name()));
        match generated {
            Some(Ok(image)) => {
                let child = client
                    .create_dynamic_artifact(image, dynamic_artifact::descriptor(self.block.id()));
                self.export_error = None;
                Some(EditorAction::OpenBlock {
                    id: child.id(),
                    block_type: Image::TYPE_ID,
                })
            }
            Some(Err(error)) => {
                self.export_error = Some(error);
                None
            }
            None => None,
        }
    }

    fn show_export_error(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.export_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        width: u16,
        height: u16,
        input_enabled: bool,
        viewport_controls: &mut DirectEditorViewport,
    ) {
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (viewport, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(viewport);
        painter.rect_filled(viewport, 0.0, ui.visuals().extreme_bg_color);

        let fit_scale = (viewport.width() / f32::from(width))
            .min(viewport.height() / f32::from(height))
            .max(f32::EPSILON);
        if input_enabled && response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    viewport_controls.change_zoom((scroll * 0.002).exp(), Some(pointer));
                }
            }
        }

        self.handle_shortcuts(ui, &response, input_enabled, viewport_controls);

        let canvas_size = Vec2::new(f32::from(width) * fit_scale, f32::from(height) * fit_scale);

        let panning = input_enabled
            && response.ctx.input(|input| {
                input.pointer.button_down(PointerButton::Middle)
                    || (input.key_down(egui::Key::Space)
                        && input.pointer.button_down(PointerButton::Primary))
            });
        if panning && response.hovered() {
            viewport_controls.pan(response.ctx.input(|input| input.pointer.delta()));
            self.active_drawing = None;
            self.committed_preview = None;
        }

        let canvas_rect = Rect::from_center_size(viewport.center(), canvas_size);
        let pointer = response
            .ctx
            .pointer_hover_pos()
            .filter(|position| viewport.contains(*position));
        let hovered_pixel =
            pointer.and_then(|position| pixel_at(position, canvas_rect, width, height));
        self.replace_source_hover = if self.tool == PixelTool::ReplaceColor {
            hovered_pixel.and_then(|(x, y)| self.block.read().and_then(|art| art.pixel(x, y)))
        } else {
            None
        };

        if response.hovered() && input_enabled {
            let cursor = if panning {
                egui::CursorIcon::Grabbing
            } else if response.ctx.input(|input| input.key_down(egui::Key::Space)) {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            };
            response.ctx.set_cursor_icon(cursor);
        }

        if !input_enabled || panning {
            self.active_drawing = None;
            self.committed_preview = None;
            self.update_texture_preview(&[], self.color);
            self.paint_edit_texture(&painter, canvas_rect);
            return;
        }

        if response.clicked_by(PointerButton::Secondary) {
            if let Some(pixel) = hovered_pixel {
                self.sample_pixel(pixel);
            }
        }

        let (primary_pressed, primary_down, primary_released, shift, space) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.modifiers.shift,
                input.key_down(egui::Key::Space),
            )
        });
        let canvas_pointer_pressed =
            primary_pressed && response.hovered() && response.is_pointer_button_down_on();
        match self.tool {
            tool if tool.is_drawing() => {
                if self.active_drawing.is_none() && canvas_pointer_pressed && !space {
                    if let Some(pixel) = hovered_pixel {
                        self.committed_preview = None;
                        self.active_drawing = Some(ActiveDrawing {
                            tool,
                            start: pixel,
                            end: pixel,
                            path: vec![pixel],
                        });
                    }
                } else if primary_down {
                    if let Some(pixel) = hovered_pixel {
                        self.extend_active_drawing(pixel);
                    }
                }
                if primary_released {
                    self.commit_active_drawing(width, height, shift);
                }
            }
            PixelTool::Fill => {
                if response.clicked_by(PointerButton::Primary) {
                    if let Some((x, y)) = hovered_pixel {
                        self.remember_color(self.color);
                        self.record_pixels(PixelArtOperation::Fill {
                            x,
                            y,
                            color: self.color,
                        });
                        ui.ctx().request_repaint();
                    }
                }
            }
            PixelTool::Eyedropper => {
                if response.clicked_by(PointerButton::Primary) {
                    if let Some(pixel) = hovered_pixel {
                        self.sample_pixel(pixel);
                    }
                }
            }
            PixelTool::ReplaceColor => {
                if response.clicked_by(PointerButton::Primary) {
                    if let Some(source) = self.replace_source_hover {
                        self.remember_color(self.color);
                        self.record_pixels(PixelArtOperation::ReplaceColor {
                            from: source,
                            to: self.color,
                        });
                        ui.ctx().request_repaint();
                    }
                }
            }
            PixelTool::Pencil
            | PixelTool::Eraser
            | PixelTool::Line
            | PixelTool::Rectangle
            | PixelTool::Ellipse => unreachable!(),
        }

        let (pending, preview_color) = if let Some(drawing) = &self.active_drawing {
            (
                rasterize_drawing(
                    drawing,
                    width,
                    height,
                    self.brush_size,
                    self.brush_shape,
                    self.shapes_filled,
                    self.mirror_horizontal,
                    self.mirror_vertical,
                    shift,
                ),
                if drawing.tool == PixelTool::Eraser {
                    PixelColor::TRANSPARENT
                } else {
                    self.color
                },
            )
        } else if let Some(preview) = &self.committed_preview {
            (preview.pixels.clone(), preview.color)
        } else if let Some(pixel) = hovered_pixel.filter(|_| self.tool.is_drawing()) {
            let drawing = ActiveDrawing {
                tool: self.tool,
                start: pixel,
                end: pixel,
                path: vec![pixel],
            };
            (
                rasterize_drawing(
                    &drawing,
                    width,
                    height,
                    self.brush_size,
                    self.brush_shape,
                    self.shapes_filled,
                    self.mirror_horizontal,
                    self.mirror_vertical,
                    shift,
                ),
                if self.tool == PixelTool::Eraser {
                    PixelColor::TRANSPARENT
                } else {
                    self.color
                },
            )
        } else {
            (Vec::new(), self.color)
        };
        self.update_texture_preview(&pending, preview_color);
        self.paint_edit_texture(&painter, canvas_rect);
        if self.show_grid {
            paint_grid(&painter, canvas_rect, width, height);
        } else {
            paint_canvas_border(&painter, canvas_rect);
        }

        if let Some(pixel) = hovered_pixel {
            paint_hovered_pixel(&painter, canvas_rect, width, height, pixel);
            let label_position = viewport.left_bottom() + Vec2::new(6.0, -6.0);
            let label = if let Some(source) = self.replace_source_hover {
                format!(
                    "{}, {} · {} {} {}",
                    pixel.0,
                    pixel.1,
                    format_hex_color(source),
                    ICON_ARROW_FORWARD.codepoint,
                    format_hex_color(self.color)
                )
            } else {
                format!("{}, {}", pixel.0, pixel.1)
            };
            painter.text(
                label_position,
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::TextStyle::Monospace.resolve(ui.style()),
                ui.visuals().text_color(),
            );
        }

        if let Some(preview) = &mut self.committed_preview {
            preview.frames_remaining = preview.frames_remaining.saturating_sub(1);
            if preview.frames_remaining == 0 {
                self.committed_preview = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
    }

    fn handle_shortcuts(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        input_enabled: bool,
        viewport: &mut DirectEditorViewport,
    ) {
        if !input_enabled || !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }

        let (
            tool,
            cancel,
            larger,
            smaller,
            toggle_fill,
            toggle_horizontal,
            toggle_vertical,
            zoom_in,
            zoom_out,
            fit,
        ) = ui.input(|input| {
            let tool = if input.key_pressed(egui::Key::B) || input.key_pressed(egui::Key::P) {
                Some(PixelTool::Pencil)
            } else if input.key_pressed(egui::Key::E) {
                Some(PixelTool::Eraser)
            } else if input.key_pressed(egui::Key::G) {
                Some(PixelTool::Fill)
            } else if input.key_pressed(egui::Key::I) {
                Some(PixelTool::Eyedropper)
            } else if input.key_pressed(egui::Key::C) {
                Some(PixelTool::ReplaceColor)
            } else if input.key_pressed(egui::Key::L) {
                Some(PixelTool::Line)
            } else if input.key_pressed(egui::Key::R) {
                Some(PixelTool::Rectangle)
            } else if input.key_pressed(egui::Key::O) {
                Some(PixelTool::Ellipse)
            } else {
                None
            };
            (
                tool,
                input.key_pressed(egui::Key::Escape),
                input.key_pressed(egui::Key::CloseBracket),
                input.key_pressed(egui::Key::OpenBracket),
                input.key_pressed(egui::Key::X),
                input.key_pressed(egui::Key::H),
                input.key_pressed(egui::Key::V),
                input.key_pressed(egui::Key::Plus),
                input.key_pressed(egui::Key::Minus),
                input.key_pressed(egui::Key::Num0),
            )
        });

        if let Some(tool) = tool {
            self.select_tool(tool);
        }
        if cancel {
            self.active_drawing = None;
        }
        if larger {
            self.brush_size = (self.brush_size + 1).min(MAX_BRUSH_SIZE);
        }
        if smaller {
            self.brush_size = self.brush_size.saturating_sub(1).max(1);
        }
        if toggle_fill && matches!(self.tool, PixelTool::Rectangle | PixelTool::Ellipse) {
            self.shapes_filled = !self.shapes_filled;
        }
        if toggle_horizontal {
            self.mirror_horizontal = !self.mirror_horizontal;
        }
        if toggle_vertical {
            self.mirror_vertical = !self.mirror_vertical;
        }
        if zoom_in {
            viewport.change_zoom(ZOOM_STEP, None);
        } else if zoom_out {
            viewport.change_zoom(1.0 / ZOOM_STEP, None);
        } else if fit {
            viewport.fit();
        }
    }

    fn extend_active_drawing(&mut self, pixel: (u16, u16)) {
        let Some(drawing) = &mut self.active_drawing else {
            return;
        };
        if drawing.end == pixel {
            return;
        }
        if matches!(drawing.tool, PixelTool::Pencil | PixelTool::Eraser) {
            drawing
                .path
                .extend(pixels_on_line(drawing.end, pixel).into_iter().skip(1));
        }
        drawing.end = pixel;
    }

    fn commit_active_drawing(&mut self, width: u16, height: u16, constrained: bool) {
        let Some(drawing) = self.active_drawing.take() else {
            return;
        };
        let pixels = rasterize_drawing(
            &drawing,
            width,
            height,
            self.brush_size,
            self.brush_shape,
            self.shapes_filled,
            self.mirror_horizontal,
            self.mirror_vertical,
            constrained,
        );
        if pixels.is_empty() {
            return;
        }
        let color = if drawing.tool == PixelTool::Eraser {
            PixelColor::TRANSPARENT
        } else {
            self.remember_color(self.color);
            self.color
        };
        self.committed_preview = Some(CommittedPreview {
            pixels: pixels.clone(),
            color,
            frames_remaining: 2,
        });
        self.record_pixels(PixelArtOperation::Paint {
            pixels: pixels
                .iter()
                .copied()
                .map(|(x, y)| PixelUpdate { x, y, color })
                .collect(),
        });
    }

    fn sample_pixel(&mut self, pixel: (u16, u16)) {
        if let Some(color) = self
            .block
            .read()
            .and_then(|art| art.pixel(pixel.0, pixel.1))
        {
            self.set_active_color(color, true);
            if self.tool == PixelTool::Eyedropper {
                self.tool = self.previous_drawing_tool;
            }
        }
    }

    fn resize_dialog(
        &mut self,
        context: &egui::Context,
        width: u16,
        height: u16,
        viewport: &mut DirectEditorViewport,
    ) {
        if !self.resize_open {
            return;
        }

        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Resize Pixel Art")
            .id(egui::Id::new(("pixel-art-resize", self.block.id())))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(
                        egui::DragValue::new(&mut self.resize_width).range(1..=MAX_PIXEL_ART_SIZE),
                    );
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut self.resize_height).range(1..=MAX_PIXEL_ART_SIZE),
                    );
                });
                ui.separator();
                ui.label("Anchor");
                anchor_selector(ui, &mut self.resize_anchor);
                if self.resize_width < width || self.resize_height < height {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "Shrinking crops pixels outside the anchored region.",
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Resize").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if apply {
            self.record_resize(self.resize_width, self.resize_height, self.resize_anchor);
            self.active_drawing = None;
            viewport.fit();
            open = false;
        } else if cancel {
            open = false;
        }
        self.resize_open = open;
    }

    fn clear_dialog(&mut self, context: &egui::Context) {
        if !self.clear_open {
            return;
        }

        let mut open = true;
        let mut clear = false;
        let mut cancel = false;
        egui::Window::new("Clear Pixel Art?")
            .id(egui::Id::new(("pixel-art-clear", self.block.id())))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                ui.label("This will make every pixel transparent.");
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        clear = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if clear {
            self.record_pixels(PixelArtOperation::Clear);
            open = false;
        } else if cancel {
            open = false;
        }
        self.clear_open = open;
    }
}

impl BlockEditor for PixelArtEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        PixelArt::TYPE_ID
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

    fn history(&self) -> Option<&dyn block_client::BlockHistoryHandle> {
        Some(&self.block)
    }

    fn render(&mut self, context: BlockRenderContext<'_>) -> bool {
        let dark_mode = context.painter.ctx().global_style().visuals.dark_mode;
        if self
            .ensure_texture(context.painter.ctx(), dark_mode)
            .is_none()
        {
            return false;
        }
        self.paint_texture(context);
        true
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        let art = self.block.read()?;
        Some(f32::from(art.width()) / f32::from(art.height()))
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }

    fn direct_editor_capabilities(&self) -> Option<DirectEditorCapabilities> {
        Some(DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: true,
        })
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<Vec2> {
        let art = self.block.read()?;
        let width = f32::from(art.width());
        let height = f32::from(art.height());
        let pixel_size = (DIRECT_EDITOR_LONG_SIDE / width.max(height))
            .max(DIRECT_EDITOR_SHORT_SIDE / width.min(height));
        Some(Vec2::new(width * pixel_size, height * pixel_size))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut super::EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let art = self.block.read()?;
        let width = art.width();
        let height = art.height();
        drop(art);
        let export = self.top_bar(ui, width, height, viewport);
        self.show_export_error(ui);
        export.then(|| self.export(editors.client())).flatten()
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut super::EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.tools_panel(ui);
        None
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut super::EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let palette = self.block.read()?.palette().to_vec();
        self.color_panel(ui, &palette);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let dark_mode = ui.visuals().dark_mode;
        let Some((width, height)) = self.ensure_texture(ui.ctx(), dark_mode) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let input_enabled = !self.resize_open && !self.clear_open;
        self.canvas(ui, width, height, input_enabled, viewport);
        self.resize_dialog(ui.ctx(), width, height, viewport);
        self.clear_dialog(ui.ctx());
        None
    }
}

fn checkerboard_image(art: &PixelArt, dark_mode: bool) -> egui::ColorImage {
    let (light, dark) = checkerboard_colors(dark_mode);
    let width = usize::from(art.width());
    let height = usize::from(art.height());
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let background = if (x + y) % 2 == 0 { light } else { dark };
            let offset = (y * width + x) * 4;
            let rgba = &art.rgba_bytes()[offset..offset + 4];
            pixels.push(composite_pixel(
                PixelColor::new(rgba[0], rgba[1], rgba[2], rgba[3]),
                background,
            ));
        }
    }
    egui::ColorImage::new([width, height], pixels)
}

fn checkerboard_colors(dark_mode: bool) -> ([u8; 3], [u8; 3]) {
    if dark_mode {
        ([82, 82, 82], [58, 58, 58])
    } else {
        ([232, 232, 232], [202, 202, 202])
    }
}

fn composite_pixel(color: PixelColor, background: [u8; 3]) -> Color32 {
    let alpha = u16::from(color.alpha);
    let inverse = 255 - alpha;
    Color32::from_rgb(
        ((u16::from(color.red) * alpha + u16::from(background[0]) * inverse) / 255) as u8,
        ((u16::from(color.green) * alpha + u16::from(background[1]) * inverse) / 255) as u8,
        ((u16::from(color.blue) * alpha + u16::from(background[2]) * inverse) / 255) as u8,
    )
}

fn format_hex_color(color: PixelColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

fn parse_hex_color(value: &str) -> Option<PixelColor> {
    let value = value.strip_prefix('#')?;
    if value.len() != 8 || !value.is_ascii() {
        return None;
    }
    Some(PixelColor::new(
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
        u8::from_str_radix(&value[6..8], 16).ok()?,
    ))
}

fn color_swatch(ui: &mut egui::Ui, color: PixelColor, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let half = rect.size() * 0.5;
        for (offset, background) in [
            (Vec2::ZERO, Color32::from_gray(220)),
            (Vec2::new(half.x, 0.0), Color32::from_gray(170)),
            (Vec2::new(0.0, half.y), Color32::from_gray(170)),
            (half, Color32::from_gray(220)),
        ] {
            painter.rect_filled(
                Rect::from_min_size(rect.min + offset, half),
                0.0,
                background,
            );
        }
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha),
        );
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(
                if selected { 2.0 } else { 1.0 },
                if selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().widgets.noninteractive.bg_stroke.color
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    ))
}

#[allow(clippy::too_many_arguments)]
fn rasterize_drawing(
    drawing: &ActiveDrawing,
    width: u16,
    height: u16,
    brush_size: u16,
    brush_shape: BrushShape,
    shapes_filled: bool,
    mirror_horizontal: bool,
    mirror_vertical: bool,
    constrained: bool,
) -> Vec<(u16, u16)> {
    let end = if constrained
        && matches!(
            drawing.tool,
            PixelTool::Line | PixelTool::Rectangle | PixelTool::Ellipse
        ) {
        constrained_endpoint(drawing.start, drawing.end, drawing.tool, width, height)
    } else {
        drawing.end
    };

    let filled_shape =
        shapes_filled && matches!(drawing.tool, PixelTool::Rectangle | PixelTool::Ellipse);
    let base = match drawing.tool {
        PixelTool::Pencil | PixelTool::Eraser => drawing.path.clone(),
        PixelTool::Line => pixels_on_line(drawing.start, end),
        PixelTool::Rectangle if filled_shape => filled_rectangle(drawing.start, end),
        PixelTool::Rectangle => rectangle_outline(drawing.start, end),
        PixelTool::Ellipse if filled_shape => ellipse_pixels(drawing.start, end, true),
        PixelTool::Ellipse => ellipse_pixels(drawing.start, end, false),
        PixelTool::Fill | PixelTool::Eyedropper | PixelTool::ReplaceColor => Vec::new(),
    };

    let mut pixels = BTreeSet::new();
    if filled_shape {
        for pixel in base {
            insert_with_symmetry(
                &mut pixels,
                pixel,
                width,
                height,
                mirror_horizontal,
                mirror_vertical,
            );
        }
    } else {
        for center in base {
            for pixel in brush_stamp(center, brush_size, brush_shape, width, height) {
                insert_with_symmetry(
                    &mut pixels,
                    pixel,
                    width,
                    height,
                    mirror_horizontal,
                    mirror_vertical,
                );
            }
        }
    }
    pixels.into_iter().collect()
}

fn constrained_endpoint(
    start: (u16, u16),
    end: (u16, u16),
    tool: PixelTool,
    width: u16,
    height: u16,
) -> (u16, u16) {
    let delta_x = i32::from(end.0) - i32::from(start.0);
    let delta_y = i32::from(end.1) - i32::from(start.1);
    let absolute_x = delta_x.abs();
    let absolute_y = delta_y.abs();
    let (sign_x, horizontal_limit) = direction_and_limit(start.0, width, delta_x);
    let (sign_y, vertical_limit) = direction_and_limit(start.1, height, delta_y);

    let (x, y) = if tool == PixelTool::Line {
        if absolute_x > absolute_y.saturating_mul(2) {
            (i32::from(end.0), i32::from(start.1))
        } else if absolute_y > absolute_x.saturating_mul(2) {
            (i32::from(start.0), i32::from(end.1))
        } else {
            let distance = absolute_x
                .max(absolute_y)
                .min(horizontal_limit)
                .min(vertical_limit);
            (
                i32::from(start.0) + sign_x * distance,
                i32::from(start.1) + sign_y * distance,
            )
        }
    } else {
        let distance = absolute_x
            .max(absolute_y)
            .min(horizontal_limit)
            .min(vertical_limit);
        (
            i32::from(start.0) + sign_x * distance,
            i32::from(start.1) + sign_y * distance,
        )
    };
    (x as u16, y as u16)
}

fn direction_and_limit(start: u16, length: u16, delta: i32) -> (i32, i32) {
    if delta < 0 {
        (-1, i32::from(start))
    } else if delta > 0 {
        (1, i32::from(length - 1 - start))
    } else {
        let negative = i32::from(start);
        let positive = i32::from(length - 1 - start);
        if positive >= negative {
            (1, positive)
        } else {
            (-1, negative)
        }
    }
}

fn brush_stamp(
    center: (u16, u16),
    size: u16,
    shape: BrushShape,
    width: u16,
    height: u16,
) -> Vec<(u16, u16)> {
    let size = i32::from(size);
    let low = -((size - 1) / 2);
    let high = size / 2;
    let radius = f64::from(size) * 0.5 - 0.25;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for offset_y in low..=high {
        for offset_x in low..=high {
            if shape == BrushShape::Circle {
                let local_x = f64::from(offset_x) - f64::from(low);
                let local_y = f64::from(offset_y) - f64::from(low);
                let center_offset = (f64::from(size) - 1.0) * 0.5;
                let delta_x = local_x - center_offset;
                let delta_y = local_y - center_offset;
                if delta_x * delta_x + delta_y * delta_y > radius * radius {
                    continue;
                }
            }
            let x = i32::from(center.0) + offset_x;
            let y = i32::from(center.1) + offset_y;
            if x >= 0 && y >= 0 && x < i32::from(width) && y < i32::from(height) {
                pixels.push((x as u16, y as u16));
            }
        }
    }
    pixels
}

fn insert_with_symmetry(
    pixels: &mut BTreeSet<(u16, u16)>,
    pixel: (u16, u16),
    width: u16,
    height: u16,
    mirror_horizontal: bool,
    mirror_vertical: bool,
) {
    let mirrored_x = width - 1 - pixel.0;
    let mirrored_y = height - 1 - pixel.1;
    pixels.insert(pixel);
    if mirror_horizontal {
        pixels.insert((mirrored_x, pixel.1));
    }
    if mirror_vertical {
        pixels.insert((pixel.0, mirrored_y));
    }
    if mirror_horizontal && mirror_vertical {
        pixels.insert((mirrored_x, mirrored_y));
    }
}

fn filled_rectangle(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut pixels =
        Vec::with_capacity(usize::from(right - left + 1) * usize::from(bottom - top + 1));
    for y in top..=bottom {
        for x in left..=right {
            pixels.push((x, y));
        }
    }
    pixels
}

fn rectangle_outline(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut pixels = BTreeSet::new();
    for x in left..=right {
        pixels.insert((x, top));
        pixels.insert((x, bottom));
    }
    for y in top..=bottom {
        pixels.insert((left, y));
        pixels.insert((right, y));
    }
    pixels.into_iter().collect()
}

fn ellipse_pixels(start: (u16, u16), end: (u16, u16), filled: bool) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut interior = BTreeSet::new();
    for y in top..=bottom {
        for x in left..=right {
            if ellipse_contains(x, y, left, right, top, bottom) {
                interior.insert((x, y));
            }
        }
    }
    let center_x = (u32::from(left) + u32::from(right)) / 2;
    let center_y = (u32::from(top) + u32::from(bottom)) / 2;
    for x in center_x as u16..=(u32::from(left) + u32::from(right)).div_ceil(2) as u16 {
        interior.insert((x, top));
        interior.insert((x, bottom));
    }
    for y in center_y as u16..=(u32::from(top) + u32::from(bottom)).div_ceil(2) as u16 {
        interior.insert((left, y));
        interior.insert((right, y));
    }
    if filled {
        return interior.into_iter().collect();
    }

    interior
        .iter()
        .copied()
        .filter(|(x, y)| {
            [
                (i32::from(*x) - 1, i32::from(*y)),
                (i32::from(*x) + 1, i32::from(*y)),
                (i32::from(*x), i32::from(*y) - 1),
                (i32::from(*x), i32::from(*y) + 1),
            ]
            .into_iter()
            .any(|(neighbor_x, neighbor_y)| {
                neighbor_x < 0
                    || neighbor_y < 0
                    || !interior.contains(&(neighbor_x as u16, neighbor_y as u16))
            })
        })
        .collect()
}

fn ellipse_contains(x: u16, y: u16, left: u16, right: u16, top: u16, bottom: u16) -> bool {
    if left == right {
        return x == left;
    }
    if top == bottom {
        return y == top;
    }
    let center_x = (f64::from(left) + f64::from(right)) * 0.5;
    let center_y = (f64::from(top) + f64::from(bottom)) * 0.5;
    let radius_x = f64::from(right - left) * 0.5;
    let radius_y = f64::from(bottom - top) * 0.5;
    let normalized_x = (f64::from(x) - center_x) / radius_x;
    let normalized_y = (f64::from(y) - center_y) / radius_y;
    normalized_x * normalized_x + normalized_y * normalized_y <= 1.0 + f64::EPSILON
}

const fn ordered(first: u16, second: u16) -> (u16, u16) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn paint_canvas_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_black_alpha(160)),
        egui::StrokeKind::Inside,
    );
}

fn paint_grid(painter: &egui::Painter, rect: Rect, width: u16, height: u16) {
    let cell_width = rect.width() / f32::from(width);
    let cell_height = rect.height() / f32::from(height);
    paint_canvas_border(painter, rect);
    if cell_width.min(cell_height) < 6.0 {
        return;
    }

    let grid = Stroke::new(1.0, Color32::from_black_alpha(60));
    for x in 1..width {
        let x = rect.left() + f32::from(x) * cell_width;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            grid,
        );
    }
    for y in 1..height {
        let y = rect.top() + f32::from(y) * cell_height;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            grid,
        );
    }
}

fn pixel_at(position: Pos2, rect: Rect, width: u16, height: u16) -> Option<(u16, u16)> {
    if !rect.contains(position) {
        return None;
    }
    let x = (((position.x - rect.left()) / rect.width()) * f32::from(width)).floor() as u16;
    let y = (((position.y - rect.top()) / rect.height()) * f32::from(height)).floor() as u16;
    Some((x.min(width - 1), y.min(height - 1)))
}

fn paint_hovered_pixel(
    painter: &egui::Painter,
    canvas: Rect,
    width: u16,
    height: u16,
    pixel: (u16, u16),
) {
    let cell_width = canvas.width() / f32::from(width);
    let cell_height = canvas.height() / f32::from(height);
    let min = Pos2::new(
        canvas.left() + f32::from(pixel.0) * cell_width,
        canvas.top() + f32::from(pixel.1) * cell_height,
    );
    let rect = Rect::from_min_size(min, Vec2::new(cell_width, cell_height));
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(2.0, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        0.0,
        Stroke::new(1.0, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
}

fn pixels_on_line(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (mut x, mut y) = (i32::from(start.0), i32::from(start.1));
    let (end_x, end_y) = (i32::from(end.0), i32::from(end.1));
    let delta_x = (end_x - x).abs();
    let step_x = if x < end_x { 1 } else { -1 };
    let delta_y = -(end_y - y).abs();
    let step_y = if y < end_y { 1 } else { -1 };
    let mut error = delta_x + delta_y;
    let mut pixels = Vec::new();

    loop {
        pixels.push((x as u16, y as u16));
        if x == end_x && y == end_y {
            break;
        }
        let doubled = error * 2;
        if doubled >= delta_y {
            error += delta_y;
            x += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y += step_y;
        }
    }
    pixels
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
