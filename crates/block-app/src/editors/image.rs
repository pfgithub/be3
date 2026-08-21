use block::BlockParent;
use block_client::{
    blocks::image::{Image, ImageOperation},
    BlockClient, BlockHandle,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use egui_material_icons::{icons::ICON_IMAGE, MaterialIcon};
use uuid::Uuid;

use super::{
    BlockEditor, BlockRenderContext, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};
use crate::platform::{FileFilter, FilePicker, PickedFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter::new(
        "Images",
        "Image",
        &[
            "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
        ],
        &["image/*"],
    )
}

const DIRECT_EDITOR_LONG_SIDE: f32 = 1024.0;
const DIRECT_EDITOR_SHORT_SIDE: f32 = 24.0;

impl EditorKind for ImageEditor {
    type Block = Image;

    const DISPLAY_NAME: &'static str = "Image";
    const ICON: MaterialIcon = ICON_IMAGE;

    fn open(_client: &BlockClient, block: BlockHandle<Image>) -> Self {
        Self::new(block)
    }
}

/// An image block is the imported file, so creating one starts by choosing it.
impl ConfigurableEditor for ImageEditor {
    type Options = ChosenImage;

    fn create(client: &BlockClient, options: ChosenImage) -> Result<Self, String> {
        let image = options.image.ok_or("Choose an image file first")?;
        Ok(Self::new(client.create_block(image)))
    }
}

#[derive(Default)]
pub(crate) struct ChosenImage {
    picker: FilePicker,
    image: Option<Image>,
    error: Option<String>,
}

impl CreationOptions for ChosenImage {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        match self.picker.poll(ui.ctx()).map(|file| file.and_then(decode)) {
            Some(Ok(image)) => {
                self.image = Some(image);
                self.error = None;
            }
            Some(Err(error)) => {
                self.image = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(ui.ctx(), &image_filter());
            }
            match &self.image {
                Some(image) => ui.label(image.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        self.image.is_some()
    }
}

pub(crate) struct ImageEditor {
    block: BlockHandle<Image>,
    texture: Option<TextureHandle>,
    texture_error: Option<String>,
    texture_revision: Option<u64>,
    picker: FilePicker,
    import_error: Option<String>,
}

impl ImageEditor {
    pub(crate) fn new(block: BlockHandle<Image>) -> Self {
        Self {
            block,
            texture: None,
            texture_error: None,
            texture_revision: None,
            picker: FilePicker::default(),
            import_error: None,
        }
    }

    fn ensure_texture(&mut self, context: &egui::Context) -> bool {
        let revision = self.block.revision();
        if self.texture_revision != Some(revision) {
            self.texture = None;
            self.texture_error = None;
            self.texture_revision = Some(revision);
        }
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

    fn poll_picker(&mut self, context: &egui::Context) {
        match self.picker.poll(context).map(|file| file.and_then(decode)) {
            Some(Ok(image)) => {
                self.block.operate(ImageOperation::Replace { image });
                self.import_error = None;
            }
            Some(Err(error)) => self.import_error = Some(error),
            None => {}
        }
    }
}

pub(crate) fn decode(file: PickedFile) -> Result<Image, String> {
    let PickedFile { name, data } = file;
    Image::from_compressed(name.clone(), data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}

pub(super) fn create_image_block(
    editors: &mut EditorAccess<'_>,
    image: Image,
    parent: Uuid,
) -> Uuid {
    let block = editors.client().create_block(image);
    let id = block.id();
    block.set_parent(BlockParent::Uuid(parent));
    editors.insert(Box::new(ImageEditor::new(block)));
    id
}

impl BlockEditor for ImageEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(
        &mut self,
        context: BlockRenderContext<'_>,
        _editors: &mut super::EditorAccess<'_>,
    ) -> bool {
        if !self.ensure_texture(context.painter.ctx()) {
            return false;
        }
        self.paint_texture(context.painter, context.corners, context.opacity);
        true
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        let image = self.block.read()?;
        (image.height() != 0).then(|| image.width() as f32 / image.height() as f32)
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let image = self.block.read()?;
        let width = image.width() as f32;
        let height = image.height() as f32;
        let scale = (DIRECT_EDITOR_LONG_SIDE / width.max(height))
            .min(1.0)
            .max(DIRECT_EDITOR_SHORT_SIDE / width.min(height));
        Some(Vec2::new(width * scale, height * scale))
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        ui.heading("Image");
        self.poll_picker(ui.ctx());
        if ui
            .add_enabled(
                !self.picker.is_open(),
                egui::Button::new("Replace image..."),
            )
            .clicked()
        {
            self.picker.open(ui.ctx(), &image_filter());
        }
        if let Some(error) = &self.import_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
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
