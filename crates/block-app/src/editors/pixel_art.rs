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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelTool {
    Pencil,
    Eraser,
}

pub(super) struct PixelArtEditor {
    block: BlockHandle<PixelArt>,
    tool: PixelTool,
    color: PixelColor,
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
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tool, PixelTool::Pencil, "Pencil");
            ui.selectable_value(&mut self.tool, PixelTool::Eraser, "Eraser");
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
            ui.weak(format!("{width} × {height}"));
        });
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

    fn canvas(&mut self, ui: &mut egui::Ui, width: u16, height: u16) {
        let available = ui.available_size();
        let aspect = f32::from(width) / f32::from(height);
        let size = if available.x / available.y.max(1.0) > aspect {
            Vec2::new(available.y * aspect, available.y)
        } else {
            Vec2::new(available.x, available.x / aspect)
        }
        .max(Vec2::splat(1.0));

        ui.vertical_centered(|ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let response = response.on_hover_cursor(egui::CursorIcon::Crosshair);
            let painter = ui.painter_at(rect);
            if let Some(texture) = &self.texture {
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            paint_grid(&painter, rect, width, height);

            if response.drag_started_by(PointerButton::Primary) {
                self.last_painted = None;
            }
            let primary_down = ui.input(|input| input.pointer.button_down(PointerButton::Primary));
            if (primary_down && response.is_pointer_button_down_on())
                || response.clicked_by(PointerButton::Primary)
            {
                if let Some(pixel) = response
                    .interact_pointer_pos()
                    .and_then(|position| pixel_at(position, rect, width, height))
                {
                    let points = self
                        .last_painted
                        .map_or_else(|| vec![pixel], |previous| pixels_on_line(previous, pixel));
                    let color = match self.tool {
                        PixelTool::Pencil => self.color,
                        PixelTool::Eraser => PixelColor::TRANSPARENT,
                    };
                    self.block.operate(PixelArtOperation::Paint {
                        pixels: points
                            .into_iter()
                            .map(|(x, y)| PixelUpdate { x, y, color })
                            .collect(),
                    });
                    self.last_painted = Some(pixel);
                    ui.ctx().request_repaint();
                }
            }
            if !primary_down {
                self.last_painted = None;
            }
        });
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
        self.canvas(ui, width, height);
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
