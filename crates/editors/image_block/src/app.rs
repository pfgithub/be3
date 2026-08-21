use std::{collections::HashMap, sync::Arc};

use block_client::{
    blocks::image::{Image, ImageOperation},
    BlockClient, BlockHandle,
};
use block_editor_plugin::{egui, EditorHost, FileFilter, FilePicker, PickedFile};
use uuid::Uuid;

const LOADING_FILL: egui::Color32 = egui::Color32::from_gray(35);
const INTRINSIC_LONG_SIDE: f32 = 1024.0;
const INTRINSIC_SHORT_SIDE: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Pane {
    Main,
    Preview,
}

#[derive(Default)]
struct Decoded {
    revision: Option<u64>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
}

struct Editing {
    host: EditorHost,
    _client: Arc<BlockClient>,
    block: BlockHandle<Image>,
}

struct Creating {
    host: EditorHost,
    client: BlockClient,
    chosen: Option<Image>,
}

#[derive(Default)]
pub struct ImageApp {
    editing: Option<Editing>,
    creation: Option<Creating>,
    picker: FilePicker,
    error: Option<String>,
    panes: HashMap<Pane, Decoded>,
}

impl block_editor_plugin::App for ImageApp {
    fn connect(&mut self, host: EditorHost, client: BlockClient, block_id: Uuid) {
        let client = Arc::new(client);
        let block = client.get_block::<Image>(block_id);
        self.editing = Some(Editing {
            host,
            _client: client,
            block,
        });
    }

    fn connect_creation(&mut self, host: EditorHost, client: BlockClient) {
        self.creation = Some(Creating {
            host,
            client,
            chosen: None,
        });
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .creation
            .as_mut()
            .ok_or("this editor is not filling in a block")?;
        let image = creation.chosen.take().ok_or("no file was chosen")?;
        Ok(creation.client.create_block(image).id())
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        let Some(Creating { host, chosen, .. }) = &mut self.creation else {
            return;
        };
        match self.picker.poll(host).map(|file| file.and_then(decode)) {
            Some(Ok(image)) => {
                host.set_creation_ready(true);
                *chosen = Some(image);
                self.error = None;
            }
            Some(Err(error)) => {
                host.set_creation_ready(false);
                *chosen = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(host, filter());
            }
            match chosen {
                Some(image) => ui.label(image.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let texture = match self.texture(Pane::Main, ui.ctx()) {
            Ok(texture) => texture,
            Err(error) => {
                ui.centered_and_justified(|ui| match error {
                    Some(error) => {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    None => {
                        ui.spinner();
                    }
                });
                return;
            }
        };
        let Some(size) = self.image_size() else {
            return;
        };
        let available = ui.available_size().max(egui::Vec2::splat(1.0));
        let aspect = size.x / size.y;
        let fitted = if available.x / available.y > aspect {
            egui::vec2(available.y * aspect, available.y)
        } else {
            egui::vec2(available.x, available.x / aspect)
        };
        let (viewport, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        paint(
            &ui.painter_at(viewport),
            &texture,
            egui::Rect::from_center_size(viewport.center(), fitted),
        );
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        match self.texture(Pane::Preview, ui.ctx()) {
            Ok(texture) => paint(ui.painter(), &texture, rect),
            Err(_) => {
                ui.painter().rect_filled(rect, 0.0, LOADING_FILL);
            }
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Image");
        let Some(editing) = &self.editing else {
            return;
        };
        match self
            .picker
            .poll(&editing.host)
            .map(|file| file.and_then(decode))
        {
            Some(Ok(image)) => {
                editing.block.operate(ImageOperation::Replace { image });
                self.error = None;
            }
            Some(Err(error)) => self.error = Some(error),
            None => {}
        }
        if ui
            .add_enabled(
                !self.picker.is_open(),
                egui::Button::new("Replace image..."),
            )
            .clicked()
        {
            self.picker.open(&editing.host, filter());
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let size = self.image_size()?;
        let scale = (INTRINSIC_LONG_SIDE / size.x.max(size.y))
            .min(1.0)
            .max(INTRINSIC_SHORT_SIDE / size.x.min(size.y));
        Some(size * scale)
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        let size = self.image_size()?;
        Some(size.x / size.y)
    }
}

impl ImageApp {
    fn image_size(&self) -> Option<egui::Vec2> {
        let image = self.editing.as_ref()?.block.read()?;
        (image.width() != 0 && image.height() != 0)
            .then(|| egui::vec2(image.width() as f32, image.height() as f32))
    }

    fn texture(
        &mut self,
        pane: Pane,
        context: &egui::Context,
    ) -> Result<egui::TextureHandle, Option<String>> {
        let Some(editing) = &self.editing else {
            return Err(None);
        };
        let revision = editing.block.revision();
        let decoded = self.panes.entry(pane).or_default();
        if decoded.revision != Some(revision) {
            *decoded = Decoded {
                revision: Some(revision),
                ..Decoded::default()
            };
        }
        if let Some(texture) = &decoded.texture {
            return Ok(texture.clone());
        }
        if decoded.error.is_some() {
            return Err(decoded.error.clone());
        }
        let Some(image) = editing.block.read() else {
            return Err(None);
        };
        let pixels = match image.decode_rgba() {
            Ok(pixels) => pixels,
            Err(error) => {
                decoded.error = Some(error);
                return Err(decoded.error.clone());
            }
        };
        let size = [image.width() as usize, image.height() as usize];
        let name = format!("image-block-{}", editing.block.id());
        drop(image);
        let texture = context.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
            egui::TextureOptions::LINEAR,
        );
        decoded.texture = Some(texture.clone());
        Ok(texture)
    }
}

fn paint(painter: &egui::Painter, texture: &egui::TextureHandle, rect: egui::Rect) {
    painter.image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}

fn filter() -> FileFilter {
    FileFilter {
        name: "Images".to_owned(),
        default_file_name: "Image".to_owned(),
        extensions: Image::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: Image::MIME_TYPES
            .iter()
            .map(|mime_type| (*mime_type).to_owned())
            .collect(),
    }
}

fn decode(file: PickedFile) -> Result<Image, String> {
    let PickedFile { name, data } = file;
    Image::from_compressed(name.clone(), data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}
