use block_client::blocks::image::ImageMetadata;
use image::GenericImageView;

pub struct Decoded {
    pub metadata: ImageMetadata,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn decode(data: &[u8]) -> Result<Decoded, String> {
    let format = image::guess_format(data).map_err(|error| error.to_string())?;
    let decoded =
        image::load_from_memory_with_format(data, format).map_err(|error| error.to_string())?;
    let (width, height) = decoded.dimensions();
    if width == 0 || height == 0 {
        return Err("image dimensions must be nonzero".into());
    }
    Ok(Decoded {
        metadata: ImageMetadata::Decoded {
            media_type: format.to_mime_type().to_owned(),
            width,
            height,
        },
        width,
        height,
        pixels: decoded.into_rgba8().into_raw(),
    })
}

#[cfg(test)]
mod tests;
