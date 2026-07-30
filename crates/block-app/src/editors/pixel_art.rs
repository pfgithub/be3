use block::{Block, BlockParent};
use block_client::{
    blocks::pixel_art::{
        PixelArt, PixelArtAnchor, PixelArtOperation, PixelColor, PixelUpdate, MAX_PIXEL_ART_SIZE,
    },
    BlockHandle, BlockRelationships,
};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use uuid::Uuid;

use super::{BlockEditor, EditorAction};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 32.0;
const ZOOM_STEP: f32 = 1.25;
const PAN_MARGIN: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
}

pub(super) struct PixelArtEditor {
    block: BlockHandle<PixelArt>,
    tool: PixelTool,
    color: PixelColor,
    zoom: f32,
    pan: Vec2,
    last_painted: Option<(u16, u16)>,
    texture: Option<TextureHandle>,
    texture_revision: Option<u64>,
    texture_size: [u16; 2],
    texture_dark_mode: bool,
    resize_open: bool,
    resize_width: u16,
    resize_height: u16,
    resize_anchor: PixelArtAnchor,
    clear_open: bool,
}

impl PixelArtEditor {
    pub(super) fn new(block: BlockHandle<PixelArt>) -> Self {
        Self {
            block,
            tool: PixelTool::Pencil,
            color: PixelColor::new(0, 0, 0, 255),
            zoom: 1.0,
            pan: Vec2::ZERO,
            last_painted: None,
            texture: None,
            texture_revision: None,
            texture_size: [0, 0],
            texture_dark_mode: false,
            resize_open: false,
            resize_width: 32,
            resize_height: 32,
            resize_anchor: PixelArtAnchor::Center,
            clear_open: false,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, width: u16, height: u16) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_value(&mut self.tool, PixelTool::Pencil, "Pencil")
                .on_hover_text("Pencil (B or P)")
                .clicked()
            {
                self.last_painted = None;
            }
            if ui
                .selectable_value(&mut self.tool, PixelTool::Eraser, "Eraser")
                .on_hover_text("Eraser (E)")
                .clicked()
            {
                self.last_painted = None;
            }
            if ui
                .selectable_value(&mut self.tool, PixelTool::Fill, "Fill")
                .on_hover_text("Fill connected pixels (G)")
                .clicked()
            {
                self.last_painted = None;
            }
            if ui
                .selectable_value(&mut self.tool, PixelTool::Eyedropper, "Eyedropper")
                .on_hover_text("Sample a pixel color (I)")
                .clicked()
            {
                self.last_painted = None;
            }
            ui.separator();

            let mut color = self.color.rgba();
            if ui
                .color_edit_button_srgba_unmultiplied(&mut color)
                .on_hover_text("Pencil color")
                .changed()
            {
                self.color = PixelColor::new(color[0], color[1], color[2], color[3]);
                self.tool = PixelTool::Pencil;
            }

            ui.separator();
            if ui.small_button("−").on_hover_text("Zoom out (-)").clicked() {
                self.change_zoom(1.0 / ZOOM_STEP);
            }
            if ui
                .button(format!("{:.0}%", self.zoom * 100.0))
                .on_hover_text("Fit canvas to viewport (0)")
                .clicked()
            {
                self.reset_view();
            }
            if ui.small_button("+").on_hover_text("Zoom in (+)").clicked() {
                self.change_zoom(ZOOM_STEP);
            }
            if ui
                .small_button("Fit")
                .on_hover_text("Fit canvas to viewport (0)")
                .clicked()
            {
                self.reset_view();
            }

            ui.separator();
            if ui.button("Resize").clicked() {
                self.resize_width = width;
                self.resize_height = height;
                self.resize_anchor = PixelArtAnchor::Center;
                self.resize_open = true;
            }
            if ui.button("Clear").clicked() {
                self.clear_open = true;
            }
            ui.separator();
            ui.weak(format!("{width} × {height} px"));
        });
    }

    fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn change_zoom(&mut self, factor: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if old_zoom > 0.0 {
            self.pan *= self.zoom / old_zoom;
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
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(context.load_texture(
                format!("pixel-art-{}", self.block.id()),
                image,
                egui::TextureOptions::NEAREST,
            ));
        }
        self.texture_revision = Some(revision);
        self.texture_size = size;
        self.texture_dark_mode = dark_mode;
    }

    fn canvas(&mut self, ui: &mut egui::Ui, width: u16, height: u16, input_enabled: bool) {
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (viewport, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(viewport);
        painter.rect_filled(viewport, 0.0, ui.visuals().extreme_bg_color);

        let fit_scale = (viewport.width() / f32::from(width))
            .min(viewport.height() / f32::from(height))
            .max(f32::EPSILON);
        let old_canvas_size = Vec2::new(
            f32::from(width) * fit_scale * self.zoom,
            f32::from(height) * fit_scale * self.zoom,
        );
        let old_canvas_rect = Rect::from_center_size(viewport.center() + self.pan, old_canvas_size);

        if input_enabled && response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let relative = (pointer - old_canvas_rect.min) / old_canvas_rect.size();
                    self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
                    let new_size = Vec2::new(
                        f32::from(width) * fit_scale * self.zoom,
                        f32::from(height) * fit_scale * self.zoom,
                    );
                    let new_min = pointer - relative * new_size;
                    self.pan = new_min + new_size * 0.5 - viewport.center();
                }
            }
        }

        self.handle_shortcuts(ui, &response, input_enabled);

        let canvas_size = Vec2::new(
            f32::from(width) * fit_scale * self.zoom,
            f32::from(height) * fit_scale * self.zoom,
        );
        constrain_pan(&mut self.pan, viewport.size(), canvas_size);

        let panning = input_enabled
            && response.ctx.input(|input| {
                input.pointer.button_down(PointerButton::Middle)
                    || (input.key_down(egui::Key::Space)
                        && input.pointer.button_down(PointerButton::Primary))
            });
        if panning && response.hovered() {
            self.pan += response.ctx.input(|input| input.pointer.delta());
            constrain_pan(&mut self.pan, viewport.size(), canvas_size);
            self.last_painted = None;
        }

        let canvas_rect = Rect::from_center_size(viewport.center() + self.pan, canvas_size);
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                canvas_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        paint_grid(&painter, canvas_rect, width, height);

        let pointer = response
            .ctx
            .pointer_hover_pos()
            .filter(|position| viewport.contains(*position));
        let hovered_pixel =
            pointer.and_then(|position| pixel_at(position, canvas_rect, width, height));
        if let Some(pixel) = hovered_pixel {
            paint_hovered_pixel(&painter, canvas_rect, width, height, pixel);
            let label_position = viewport.left_bottom() + Vec2::new(6.0, -6.0);
            painter.text(
                label_position,
                egui::Align2::LEFT_BOTTOM,
                format!("{}, {}", pixel.0, pixel.1),
                egui::TextStyle::Monospace.resolve(ui.style()),
                ui.visuals().text_color(),
            );
        }

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
            self.last_painted = None;
            return;
        }

        if response.clicked_by(PointerButton::Secondary) {
            if let Some(pixel) = hovered_pixel {
                self.sample_pixel(pixel);
            }
        }

        if response.drag_started_by(PointerButton::Primary) {
            self.last_painted = None;
        }
        let primary_down = ui.input(|input| input.pointer.button_down(PointerButton::Primary));
        match self.tool {
            PixelTool::Pencil | PixelTool::Eraser => {
                if (primary_down && response.is_pointer_button_down_on())
                    || response.clicked_by(PointerButton::Primary)
                {
                    if let Some(pixel) = hovered_pixel {
                        self.paint_to(pixel);
                        ui.ctx().request_repaint();
                    }
                }
            }
            PixelTool::Fill => {
                if response.clicked_by(PointerButton::Primary) {
                    if let Some((x, y)) = hovered_pixel {
                        self.block.operate(PixelArtOperation::Fill {
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
        }
        if !primary_down {
            self.last_painted = None;
        }
    }

    fn handle_shortcuts(&mut self, ui: &egui::Ui, response: &egui::Response, input_enabled: bool) {
        if !input_enabled || !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }

        ui.input(|input| {
            if input.key_pressed(egui::Key::B) || input.key_pressed(egui::Key::P) {
                self.tool = PixelTool::Pencil;
                self.last_painted = None;
            } else if input.key_pressed(egui::Key::E) {
                self.tool = PixelTool::Eraser;
                self.last_painted = None;
            } else if input.key_pressed(egui::Key::G) {
                self.tool = PixelTool::Fill;
                self.last_painted = None;
            } else if input.key_pressed(egui::Key::I) {
                self.tool = PixelTool::Eyedropper;
                self.last_painted = None;
            }

            if input.key_pressed(egui::Key::Plus) {
                self.change_zoom(ZOOM_STEP);
            } else if input.key_pressed(egui::Key::Minus) {
                self.change_zoom(1.0 / ZOOM_STEP);
            } else if input.key_pressed(egui::Key::Num0) {
                self.reset_view();
            }
        });
    }

    fn paint_to(&mut self, pixel: (u16, u16)) {
        let points = match self.last_painted {
            None => vec![pixel],
            Some(previous) if previous == pixel => Vec::new(),
            Some(previous) => pixels_on_line(previous, pixel)
                .into_iter()
                .skip(1)
                .collect(),
        };
        if points.is_empty() {
            return;
        }

        let color = match self.tool {
            PixelTool::Eraser => PixelColor::TRANSPARENT,
            PixelTool::Pencil | PixelTool::Fill | PixelTool::Eyedropper => self.color,
        };
        self.block.operate(PixelArtOperation::Paint {
            pixels: points
                .into_iter()
                .map(|(x, y)| PixelUpdate { x, y, color })
                .collect(),
        });
        self.last_painted = Some(pixel);
    }

    fn sample_pixel(&mut self, pixel: (u16, u16)) {
        if let Some(color) = self
            .block
            .read()
            .and_then(|art| art.pixel(pixel.0, pixel.1))
        {
            self.color = color;
        }
    }

    fn resize_dialog(&mut self, context: &egui::Context, width: u16, height: u16) {
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
            self.block.operate(PixelArtOperation::Resize {
                width: self.resize_width,
                height: self.resize_height,
                anchor: self.resize_anchor,
            });
            self.last_painted = None;
            self.reset_view();
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
            self.block.operate(PixelArtOperation::Clear);
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

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        let Some(art) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let width = art.width();
        let height = art.height();
        let revision = art.revision();
        let size = [width, height];
        let dark_mode = ui.visuals().dark_mode;
        let image = (self.texture_revision != Some(revision)
            || self.texture_size != size
            || self.texture_dark_mode != dark_mode)
            .then(|| checkerboard_image(&art, dark_mode));
        drop(art);
        if let Some(image) = image {
            self.update_texture(ui.ctx(), image, revision, size, dark_mode);
        }

        self.toolbar(ui, width, height);
        ui.separator();
        let input_enabled = !self.resize_open && !self.clear_open;
        self.canvas(ui, width, height, input_enabled);
        self.resize_dialog(ui.ctx(), width, height);
        self.clear_dialog(ui.ctx());
        None
    }
}

fn checkerboard_image(art: &PixelArt, dark_mode: bool) -> egui::ColorImage {
    let (light, dark): ([u8; 3], [u8; 3]) = if dark_mode {
        ([82, 82, 82], [58, 58, 58])
    } else {
        ([232, 232, 232], [202, 202, 202])
    };
    let width = usize::from(art.width());
    let height = usize::from(art.height());
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let background = if (x + y) % 2 == 0 { light } else { dark };
            let offset = (y * width + x) * 4;
            let rgba = &art.rgba_bytes()[offset..offset + 4];
            let alpha = u16::from(rgba[3]);
            let inverse = 255 - alpha;
            pixels.push(Color32::from_rgb(
                ((u16::from(rgba[0]) * alpha + u16::from(background[0]) * inverse) / 255) as u8,
                ((u16::from(rgba[1]) * alpha + u16::from(background[1]) * inverse) / 255) as u8,
                ((u16::from(rgba[2]) * alpha + u16::from(background[2]) * inverse) / 255) as u8,
            ));
        }
    }
    egui::ColorImage::new([width, height], pixels)
}

fn paint_grid(painter: &egui::Painter, rect: Rect, width: u16, height: u16) {
    let cell_width = rect.width() / f32::from(width);
    let cell_height = rect.height() / f32::from(height);
    let border = Stroke::new(1.0, Color32::from_black_alpha(160));
    painter.rect_stroke(rect, 0.0, border, egui::StrokeKind::Inside);
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

fn constrain_pan(pan: &mut Vec2, viewport: Vec2, canvas: Vec2) {
    if canvas.x <= viewport.x {
        pan.x = 0.0;
    } else {
        let limit = (canvas.x - viewport.x) * 0.5 + PAN_MARGIN.min(viewport.x * 0.5);
        pan.x = pan.x.clamp(-limit, limit);
    }
    if canvas.y <= viewport.y {
        pan.y = 0.0;
    } else {
        let limit = (canvas.y - viewport.y) * 0.5 + PAN_MARGIN.min(viewport.y * 0.5);
        pan.y = pan.y.clamp(-limit, limit);
    }
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
            (PixelArtAnchor::TopLeft, "↖"),
            (PixelArtAnchor::Top, "↑"),
            (PixelArtAnchor::TopRight, "↗"),
        ],
        [
            (PixelArtAnchor::Left, "←"),
            (PixelArtAnchor::Center, "•"),
            (PixelArtAnchor::Right, "→"),
        ],
        [
            (PixelArtAnchor::BottomLeft, "↙"),
            (PixelArtAnchor::Bottom, "↓"),
            (PixelArtAnchor::BottomRight, "↘"),
        ],
    ] {
        ui.horizontal(|ui| {
            for (value, label) in row {
                ui.selectable_value(anchor, value, label);
            }
        });
    }
}
