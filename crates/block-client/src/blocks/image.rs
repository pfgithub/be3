use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use image::GenericImageView;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Image {
    source_name: String,
    media_type: String,
    width: u32,
    height: u32,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ImageOperation {
    Replace { image: Image },
}

#[derive(Deserialize)]
struct ImageData {
    source_name: String,
    media_type: String,
    width: u32,
    height: u32,
    #[serde(deserialize_with = "deserialize_data")]
    data: Vec<u8>,
}

impl Image {
    pub fn from_compressed(source_name: impl Into<String>, data: Vec<u8>) -> Result<Self, String> {
        let format = image::guess_format(&data).map_err(|error| error.to_string())?;
        let decoded = image::load_from_memory_with_format(&data, format)
            .map_err(|error| error.to_string())?;
        let (width, height) = decoded.dimensions();
        if width == 0 || height == 0 {
            return Err("image dimensions must be nonzero".into());
        }
        Ok(Self {
            source_name: source_name.into(),
            media_type: format.to_mime_type().into(),
            width,
            height,
            data,
        })
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl<'de> Deserialize<'de> for Image {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = ImageData::deserialize(deserializer)?;
        let image = Self::from_compressed(data.source_name, data.data).map_err(D::Error::custom)?;
        if image.media_type != data.media_type
            || image.width != data.width
            || image.height != data.height
        {
            return Err(D::Error::custom(
                "image metadata does not match compressed data",
            ));
        }
        Ok(image)
    }
}

impl Block for Image {
    type Operation = ImageOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696d_6167_652d_626c_6f63_6b2d_7479_7001);

    fn apply_operation(image: &mut Self, operation: &Self::Operation) {
        match operation {
            ImageOperation::Replace { image: replacement } => *image = replacement.clone(),
        }
    }

    fn implicit_name(&self) -> String {
        let name = self.source_name.trim();
        if name.is_empty() {
            "Image".into()
        } else {
            name.into()
        }
    }
}

fn serialize_data<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(data))
}

fn deserialize_data<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests;
