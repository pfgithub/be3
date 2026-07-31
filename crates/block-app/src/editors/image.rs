use block::{Block, BlockParent};
use block_client::{blocks::image::Image, BlockHandle, BlockRelationships};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use uuid::Uuid;

use super::{BlockEditor, BlockRenderContext, EditorAccess, EditorAction};

pub(super) struct ImageEditor {
    block: BlockHandle<Image>,
    texture: Option<TextureHandle>,
    texture_error: Option<String>,
}

impl ImageEditor {
    pub(super) fn new(block: BlockHandle<Image>) -> Self {
        Self {
            block,
            texture: None,
            texture_error: None,
        }
    }

    fn ensure_texture(&mut self, context: &egui::Context) -> bool {
        if self.texture.is_some() {
            return true;
        }
        if self.texture_error.is_some() {
            return false;
        }
        let Some(image) = self.block.read() else {
            return false;
        };
        let decoded = match image::load_from_memory(image.data()) {
            Ok(decoded) => decoded.into_rgba8(),
            Err(error) => {
                self.texture_error = Some(error.to_string());
                return false;
            }
        };
        let size = [decoded.width() as usize, decoded.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        drop(image);
        self.texture = Some(context.load_texture(
            format!("image-block-{}", self.block.id()),
            color_image,
            egui::TextureOptions::LINEAR,
        ));
        true
    }

    fn paint_texture(&self, painter: &egui::Painter, corners: [Pos2; 4], opacity: f32) {
        let Some(texture) = &self.texture else {
            return;
        };
        let tint = Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut mesh = egui::Mesh::with_texture(texture.id());
        mesh.vertices.extend([
            egui::epaint::Vertex {
                pos: corners[0],
                uv: Pos2::new(0.0, 0.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: corners[1],
                uv: Pos2::new(1.0, 0.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: corners[2],
                uv: Pos2::new(1.0, 1.0),
                color: tint,
            },
            egui::epaint::Vertex {
                pos: corners[3],
                uv: Pos2::new(0.0, 1.0),
                color: tint,
            },
        ]);
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        painter.add(mesh);
    }
}

impl BlockEditor for ImageEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        Image::TYPE_ID
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

    fn render(&mut self, context: BlockRenderContext<'_>) -> bool {
        if !self.ensure_texture(context.painter.ctx()) {
            return false;
        }
        self.paint_texture(context.painter, context.corners, context.opacity);
        true
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        if !self.ensure_texture(ui.ctx()) {
            ui.centered_and_justified(|ui| {
                if let Some(error) = &self.texture_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                } else {
                    ui.spinner();
                }
            });
            return None;
        }
        let image = self.block.read().unwrap();
        let aspect = image.width() as f32 / image.height() as f32;
        drop(image);
        let available = ui.available_size().max(Vec2::splat(1.0));
        let size = if available.x / available.y > aspect {
            Vec2::new(available.y * aspect, available.y)
        } else {
            Vec2::new(available.x, available.x / aspect)
        };
        let (viewport, _) = ui.allocate_exact_size(available, Sense::hover());
        let rect = Rect::from_center_size(viewport.center(), size);
        self.paint_texture(
            &ui.painter_at(viewport),
            [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            1.0,
        );
        None
    }
}
