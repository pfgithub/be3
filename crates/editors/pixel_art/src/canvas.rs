use block_client::blocks::pixel_art::{PixelArt, PixelArtOperation, PixelColor, PixelUpdate};
use block_editor_plugin::{
    egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, Vec2},
    egui_material_icons::icons::ICON_ARROW_FORWARD,
};
use std::collections::BTreeSet;

use crate::{
    app::{PaneKey, PixelArtApp},
    color::{checkerboard_colors, checkerboard_image, composite_pixel, format_hex_color},
    drawing::{rasterize_drawing, ActiveDrawing, CommittedPreview, PixelTool, MAX_BRUSH_SIZE},
};

pub const ZOOM_STEP: f32 = 1.25;
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 32.0;

/// Where the artwork sits in the region the editor is given: a multiple of the
/// scale that fits it, and an offset from the centre.
#[derive(Clone, Copy)]
pub struct View {
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

impl View {
    pub fn change_zoom(&mut self, factor: f32, anchor: Option<(Pos2, Rect)>) {
        let previous = self.zoom;
        let zoom = (previous * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if zoom == previous {
            return;
        }
        if let Some((pointer, region)) = anchor {
            let offset = pointer - region.center();
            self.pan = offset - (offset - self.pan) * (zoom / previous);
        }
        self.zoom = zoom;
    }
}

pub fn canvas_rect(region: Rect, width: u16, height: u16, view: View) -> Rect {
    let scale = (region.width() / f32::from(width))
        .min(region.height() / f32::from(height))
        .max(f32::EPSILON);
    Rect::from_center_size(
        region.center() + view.pan,
        Vec2::new(f32::from(width), f32::from(height)) * scale * view.zoom,
    )
}

/// The artwork as one region's context holds it: an image composited over the
/// checkerboard, and the pixels a gesture is about to paint on top of it.
#[derive(Default)]
pub struct Pane {
    texture: Option<egui::TextureHandle>,
    image: Option<egui::ColorImage>,
    preview: Vec<(u16, u16)>,
    preview_color: Option<PixelColor>,
    revision: Option<u64>,
    size: [u16; 2],
    dark_mode: bool,
}

impl Pane {
    pub fn ensure(&mut self, context: &egui::Context, art: &PixelArt, dark_mode: bool) {
        let size = [art.width(), art.height()];
        if self.revision == Some(art.revision())
            && self.size == size
            && self.dark_mode == dark_mode
            && self.texture.is_some()
        {
            return;
        }
        let image = checkerboard_image(art, dark_mode);
        match &mut self.texture {
            Some(texture) => texture.set(image.clone(), egui::TextureOptions::NEAREST),
            None => {
                self.texture = Some(context.load_texture(
                    "pixel-art",
                    image.clone(),
                    egui::TextureOptions::NEAREST,
                ));
            }
        }
        self.image = Some(image);
        self.preview.clear();
        self.preview_color = None;
        self.revision = Some(art.revision());
        self.size = size;
        self.dark_mode = dark_mode;
    }

    /// Patches the pixels a gesture would paint into the texture, and puts
    /// back the ones it no longer covers.
    pub fn set_preview(&mut self, pixels: &[(u16, u16)], color: PixelColor) {
        if self.preview == pixels && (pixels.is_empty() || self.preview_color == Some(color)) {
            return;
        }
        let (Some(texture), Some(image)) = (&mut self.texture, &self.image) else {
            return;
        };

        let mut changed = BTreeSet::new();
        changed.extend(self.preview.iter().map(|&(x, y)| (y, x)));
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
                        let (light, dark) = checkerboard_colors(self.dark_mode);
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
        self.preview = pixels.to_vec();
        self.preview_color = (!pixels.is_empty()).then_some(color);
    }

    pub fn paint(&self, painter: &egui::Painter, rect: Rect) {
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

impl PixelArtApp {
    /// The whole main region: the artwork, the gesture being drawn on it, and
    /// every way of moving around it.
    pub(crate) fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (region, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(region);
        painter.rect_filled(region, 0.0, ui.visuals().extreme_bg_color);

        let dark_mode = ui.visuals().dark_mode;
        let Some((width, height)) = self.refresh_pane(ui.ctx(), PaneKey::Main, dark_mode) else {
            ui.scope_builder(egui::UiBuilder::new().max_rect(region), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            });
            return;
        };

        let input_enabled = !self.resize_open && !self.clear_open;
        // The host cannot gate a plugin's own surface, so a block being read
        // is kept out of every gesture that would change it.
        let editable = input_enabled && self.editable();
        if input_enabled {
            self.handle_view_input(&response, region);
            self.handle_shortcuts(ui, &response);
        }

        let canvas = canvas_rect(region, width, height, self.view);
        let pointer = response
            .ctx
            .pointer_hover_pos()
            .filter(|position| region.contains(*position));
        let hovered_pixel = pointer.and_then(|position| pixel_at(position, canvas, width, height));
        self.replace_source_hover = if self.tool == PixelTool::ReplaceColor {
            hovered_pixel.and_then(|(x, y)| {
                self.editing
                    .as_ref()
                    .and_then(|editing| editing.block.read().and_then(|art| art.pixel(x, y)))
            })
        } else {
            None
        };

        let panning = input_enabled && self.panning(&response);
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

        if !editable || panning {
            self.active_drawing = None;
            self.committed_preview = None;
            self.preview_pixels(PaneKey::Main, &[], self.color);
            self.paint_pane(PaneKey::Main, &painter, canvas);
            return;
        }

        self.handle_tool_input(ui, &response, hovered_pixel, width, height);

        let (pending, preview_color) = self.pending_pixels(hovered_pixel, width, height, ui);
        self.preview_pixels(PaneKey::Main, &pending, preview_color);
        self.paint_pane(PaneKey::Main, &painter, canvas);
        if self.show_grid {
            paint_grid(&painter, canvas, width, height);
        } else {
            paint_canvas_border(&painter, canvas);
        }

        if let Some(pixel) = hovered_pixel {
            paint_hovered_pixel(&painter, canvas, width, height, pixel);
            let label = match self.replace_source_hover {
                Some(source) => format!(
                    "{}, {} · {} {} {}",
                    pixel.0,
                    pixel.1,
                    format_hex_color(source),
                    ICON_ARROW_FORWARD.codepoint,
                    format_hex_color(self.color)
                ),
                None => format!("{}, {}", pixel.0, pixel.1),
            };
            painter.text(
                region.left_bottom() + Vec2::new(6.0, -6.0),
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

    fn panning(&self, response: &egui::Response) -> bool {
        let held = response.ctx.input(|input| {
            input.pointer.button_down(PointerButton::Middle)
                || (input.key_down(egui::Key::Space)
                    && input.pointer.button_down(PointerButton::Primary))
        });
        held && response.hovered()
    }

    /// Panning and zooming, which this editor does itself because the host
    /// hands it the whole viewport.
    fn handle_view_input(&mut self, response: &egui::Response, region: Rect) {
        if self.panning(response) {
            self.view.pan += response.ctx.input(|input| input.pointer.delta());
            self.active_drawing = None;
            self.committed_preview = None;
        }
        if !response.hovered() {
            return;
        }
        let Some(pointer) = response.ctx.pointer_hover_pos() else {
            return;
        };
        let (scroll, zoom_delta) = response
            .ctx
            .input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        if (zoom_delta - 1.0).abs() > f32::EPSILON {
            self.view.change_zoom(zoom_delta, Some((pointer, region)));
        } else if scroll != 0.0 {
            self.view
                .change_zoom((scroll * 0.002).exp(), Some((pointer, region)));
        }
    }

    fn handle_tool_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        hovered_pixel: Option<(u16, u16)>,
        width: u16,
        height: u16,
    ) {
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
        let pressed_here =
            primary_pressed && response.hovered() && response.is_pointer_button_down_on();
        match self.tool {
            tool if tool.is_drawing() => {
                if self.active_drawing.is_none() && pressed_here && !space {
                    if let Some(pixel) = hovered_pixel {
                        self.committed_preview = None;
                        self.active_drawing = Some(ActiveDrawing::new(tool, pixel));
                    }
                } else if primary_down {
                    if let (Some(drawing), Some(pixel)) =
                        (self.active_drawing.as_mut(), hovered_pixel)
                    {
                        drawing.extend(pixel);
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
                        self.operate(PixelArtOperation::Fill {
                            x,
                            y,
                            color: self.color,
                        });
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
                        self.operate(PixelArtOperation::ReplaceColor {
                            from: source,
                            to: self.color,
                        });
                    }
                }
            }
            PixelTool::Pencil
            | PixelTool::Eraser
            | PixelTool::Line
            | PixelTool::Rectangle
            | PixelTool::Ellipse => unreachable!(),
        }
    }

    /// The pixels shown on top of the artwork: the gesture being drawn, the
    /// one just committed, or where the brush would land.
    fn pending_pixels(
        &self,
        hovered_pixel: Option<(u16, u16)>,
        width: u16,
        height: u16,
        ui: &egui::Ui,
    ) -> (Vec<(u16, u16)>, PixelColor) {
        let constrained = ui.input(|input| input.modifiers.shift);
        if let Some(drawing) = &self.active_drawing {
            return (
                rasterize_drawing(drawing, width, height, self.brush(constrained)),
                self.stroke_color(drawing.tool),
            );
        }
        if let Some(preview) = &self.committed_preview {
            return (preview.pixels.clone(), preview.color);
        }
        match hovered_pixel.filter(|_| self.tool.is_drawing()) {
            Some(pixel) => {
                let drawing = ActiveDrawing::new(self.tool, pixel);
                (
                    rasterize_drawing(&drawing, width, height, self.brush(constrained)),
                    self.stroke_color(self.tool),
                )
            }
            None => (Vec::new(), self.color),
        }
    }

    fn stroke_color(&self, tool: PixelTool) -> PixelColor {
        if tool == PixelTool::Eraser {
            PixelColor::TRANSPARENT
        } else {
            self.color
        }
    }

    fn commit_active_drawing(&mut self, width: u16, height: u16, constrained: bool) {
        let Some(drawing) = self.active_drawing.take() else {
            return;
        };
        let pixels = rasterize_drawing(&drawing, width, height, self.brush(constrained));
        if pixels.is_empty() {
            return;
        }
        let color = self.stroke_color(drawing.tool);
        if drawing.tool != PixelTool::Eraser {
            self.remember_color(self.color);
        }
        self.committed_preview = Some(CommittedPreview {
            pixels: pixels.clone(),
            color,
            frames_remaining: 2,
        });
        self.operate(PixelArtOperation::Paint {
            pixels: pixels
                .iter()
                .copied()
                .map(|(x, y)| PixelUpdate { x, y, color })
                .collect(),
        });
    }

    fn sample_pixel(&mut self, pixel: (u16, u16)) {
        let sampled = self.editing.as_ref().and_then(|editing| {
            editing
                .block
                .read()
                .and_then(|art| art.pixel(pixel.0, pixel.1))
        });
        if let Some(color) = sampled {
            self.set_active_color(color, true);
            if self.tool == PixelTool::Eyedropper {
                self.tool = self.previous_drawing_tool;
            }
        }
    }

    fn handle_shortcuts(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }

        let (tool, cancel, larger, smaller, toggle_fill, horizontal, vertical, zoom) =
            ui.input(|input| {
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
                let zoom = if input.key_pressed(egui::Key::Plus) {
                    Some(Some(ZOOM_STEP))
                } else if input.key_pressed(egui::Key::Minus) {
                    Some(Some(1.0 / ZOOM_STEP))
                } else if input.key_pressed(egui::Key::Num0) {
                    Some(None)
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
                    zoom,
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
        if horizontal {
            self.mirror_horizontal = !self.mirror_horizontal;
        }
        if vertical {
            self.mirror_vertical = !self.mirror_vertical;
        }
        match zoom {
            Some(Some(factor)) => self.view.change_zoom(factor, None),
            Some(None) => self.view = View::default(),
            None => {}
        }
    }
}

pub fn pixel_at(position: Pos2, rect: Rect, width: u16, height: u16) -> Option<(u16, u16)> {
    if !rect.contains(position) {
        return None;
    }
    let x = (((position.x - rect.left()) / rect.width()) * f32::from(width)).floor() as u16;
    let y = (((position.y - rect.top()) / rect.height()) * f32::from(height)).floor() as u16;
    Some((x.min(width - 1), y.min(height - 1)))
}

fn paint_canvas_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::from_black_alpha(160)),
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

    let grid = Stroke::new(1.0_f32, Color32::from_black_alpha(60));
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
        Stroke::new(2.0_f32, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(2.0),
        0.0,
        Stroke::new(1.0_f32, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
}
