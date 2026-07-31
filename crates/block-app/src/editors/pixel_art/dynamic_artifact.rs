use block::Block;
use block_client::{
    blocks::{image::Image, pixel_art::PixelArt},
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use uuid::Uuid;

use super::super::DynamicArtifactRegeneration;

pub(super) fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: PixelArt::TYPE_ID,
        data: source_id.as_bytes().to_vec(),
    }
}

pub(super) fn generate(art: &PixelArt, source_name: &str) -> Result<Image, String> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            art.rgba_bytes(),
            u32::from(art.width()),
            u32::from(art.height()),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Image::from_compressed(format!("{source_name} Export"), png)
}

pub(in crate::editors) fn regenerate(
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
    let source_id = Uuid::from_slice(data)
        .map_err(|_| "pixel art export descriptor has an invalid source id".to_owned())?;
    Ok(Box::new(PixelArtRegeneration {
        source: client.get_block::<PixelArt>(source_id),
        target: client.get_block::<Image>(target_id),
    }))
}

struct PixelArtRegeneration {
    source: BlockHandle<PixelArt>,
    target: BlockHandle<Image>,
}

impl DynamicArtifactRegeneration for PixelArtRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;
        if self.target.read().is_none() {
            return None;
        }
        let generated = generate(&source, &self.source.name());
        drop(source);
        Some(generated.map(|image| self.target.replace(image)))
    }
}

#[cfg(test)]
#[path = "dynamic_artifact/tests/pixel_art_export_generates_png.rs"]
mod pixel_art_export_generates_png;
