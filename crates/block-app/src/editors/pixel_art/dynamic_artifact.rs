use block::Block;
use block_client::{
    blocks::{image::Image, pixel_art::PixelArt},
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use eframe::egui;
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::{DynamicArtifactRegeneration, DynamicArtifactSupport};

const MAX_EXPORT_SCALE: u32 = 16;

/// The descriptor payload: which drawing the image came from, and how it is
/// exported.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageArtifact {
    source: Uuid,
    settings: ImageSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageSettings {
    /// Nearest-neighbour magnification, so exports stay crisp when scaled.
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

pub(in crate::editors) const SUPPORT: DynamicArtifactSupport = DynamicArtifactSupport {
    source: |data| ImageArtifact::decode(data).map(|artifact| artifact.source),
    summary,
    settings_ui,
    regenerate,
};

pub(super) fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: PixelArt::TYPE_ID,
        data: ImageArtifact {
            source: source_id,
            settings: ImageSettings::default(),
        }
        .encode(),
    }
}

/// The image a freshly exported artifact starts with.
pub(super) fn generate_initial(art: &PixelArt, source_name: &str) -> Result<Image, String> {
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
    Image::from_compressed(format!("{source_name} Export"), png)
}

/// Repeats every pixel `scale` times in both directions.
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
        for pixel in row.chunks_exact(4) {
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

fn summary(data: &[u8]) -> String {
    let Ok(artifact) = ImageArtifact::decode(data) else {
        return "PNG export".to_owned();
    };
    let scale = artifact.settings.scale.clamp(1, MAX_EXPORT_SCALE);
    if scale == 1 {
        "PNG export at the original size".to_owned()
    } else {
        format!("PNG export at {scale}x")
    }
}

fn settings_ui(ui: &mut egui::Ui, data: &mut Vec<u8>) -> bool {
    let Ok(mut artifact) = ImageArtifact::decode(data) else {
        ui.label("These settings cannot be read.");
        return false;
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
    if changed {
        *data = artifact.encode();
    }
    changed
}

fn regenerate(
    client: &BlockClient,
    target_id: Uuid,
    target_type: Uuid,
    data: &[u8],
) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
    if target_type != Image::TYPE_ID {
        return Err(format!(
            "pixel art export expected an Image target, found {target_type}"
        ));
    }
    let artifact = ImageArtifact::decode(data)?;
    Ok(Box::new(PixelArtRegeneration {
        source: client.get_block::<PixelArt>(artifact.source),
        target: client.get_block::<Image>(target_id),
        settings: artifact.settings,
    }))
}

struct PixelArtRegeneration {
    source: BlockHandle<PixelArt>,
    target: BlockHandle<Image>,
    settings: ImageSettings,
}

impl DynamicArtifactRegeneration for PixelArtRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>> {
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
