use block::Block;
use block_client::{
    blocks::{image::Image, pixel_art::PixelArt},
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use block_editor_plugin::{egui, ArtifactDescription};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_EXPORT_SCALE: u32 = 16;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageArtifact {
    source: Uuid,
    settings: ImageSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageSettings {
    scale: u32,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self { scale: 1 }
    }
}

impl ImageArtifact {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("pixel art export descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: PixelArt::TYPE_ID,
        data: ImageArtifact {
            source: source_id,
            settings: ImageSettings::default(),
        }
        .encode(),
    }
}

pub fn generate_initial(art: &PixelArt, source_name: &str) -> Result<Image, String> {
    generate(art, source_name, &ImageSettings::default())
}

fn generate(art: &PixelArt, source_name: &str, settings: &ImageSettings) -> Result<Image, String> {
    let scale = settings.scale.clamp(1, MAX_EXPORT_SCALE);
    let width = u32::from(art.width()) * scale;
    let height = u32::from(art.height()) * scale;
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &magnified(art, scale),
            width,
            height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(Image::new(format!("{source_name} Export"), png))
}

fn magnified(art: &PixelArt, scale: u32) -> Vec<u8> {
    let pixels = art.rgba_bytes();
    if scale == 1 {
        return pixels.to_vec();
    }
    let width = usize::from(art.width());
    let scale = scale as usize;
    let mut magnified = Vec::with_capacity(pixels.len() * scale * scale);
    for row in pixels.chunks_exact(width * 4) {
        let start = magnified.len();
        for pixel in row.as_chunks::<4>().0 {
            for _ in 0..scale {
                magnified.extend_from_slice(pixel);
            }
        }
        let magnified_row = magnified[start..].to_vec();
        for _ in 1..scale {
            magnified.extend_from_slice(&magnified_row);
        }
    }
    magnified
}

pub fn describe(data: &[u8]) -> Result<ArtifactDescription, String> {
    let artifact = ImageArtifact::decode(data)?;
    Ok(ArtifactDescription {
        source: artifact.source,
        summary: summary(&artifact.settings),
    })
}

fn summary(settings: &ImageSettings) -> String {
    let scale = settings.scale.clamp(1, MAX_EXPORT_SCALE);
    if scale == 1 {
        "PNG export at the original size".to_owned()
    } else {
        format!("PNG export at {scale}x")
    }
}

pub fn settings_ui(ui: &mut egui::Ui, data: &mut Vec<u8>) {
    let Ok(mut artifact) = ImageArtifact::decode(data) else {
        ui.label("These settings cannot be read.");
        return;
    };
    let changed = ui
        .horizontal(|ui| {
            ui.label("Scale");
            ui.add(
                egui::DragValue::new(&mut artifact.settings.scale)
                    .range(1..=MAX_EXPORT_SCALE)
                    .suffix("x"),
            )
            .changed()
        })
        .inner;
    ui.add_space(12.0);
    ui.weak(summary(&artifact.settings));
    if changed {
        *data = artifact.encode();
    }
}

pub struct Regeneration {
    source: BlockHandle<PixelArt>,
    target: BlockHandle<Image>,
    settings: ImageSettings,
}

impl Regeneration {
    pub fn start(
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Self, String> {
        if target_type != Image::TYPE_ID {
            return Err(format!(
                "pixel art export expected an Image target, found {target_type}"
            ));
        }
        let artifact = ImageArtifact::decode(data)?;
        Ok(Self {
            source: client.get_block::<PixelArt>(artifact.source),
            target: client.get_block::<Image>(target_id),
            settings: artifact.settings,
        })
    }

    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;
        self.target.read()?;
        let name = self.source.name().unwrap_or_else(|| "Pixel Art".to_owned());
        let generated = generate(&source, &name, &self.settings);
        drop(source);
        Some(generated.map(|image| self.target.replace(image)))
    }
}

#[cfg(test)]
mod tests;
