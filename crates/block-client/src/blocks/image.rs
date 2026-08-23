use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Image {
    source_name: String,
    metadata: ImageMetadata,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

                                                                              
                               
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum ImageMetadata {
    #[default]
    Undecoded,
    Decoded {
        media_type: String,
        width: u32,
        height: u32,
    },
    Failed(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ImageOperation {
    Replace { image: Image },
    SetMetadata { metadata: ImageMetadata },
}

impl Image {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &[
        "bmp", "gif", "ico", "jpg", "jpeg", "png", "pnm", "tga", "tif", "tiff", "webp",
    ];
    pub const MIME_TYPES: &'static [&'static str] = &["image/*"];

    pub fn new(source_name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            source_name: source_name.into(),
            metadata: ImageMetadata::Undecoded,
            data,
        }
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub const fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        match self.metadata {
            ImageMetadata::Decoded { width, height, .. } => Some((width, height)),
            ImageMetadata::Undecoded | ImageMetadata::Failed(_) => None,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Block for Image {
    type Operation = ImageOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696d_6167_652d_626c_6f63_6b2d_7479_7001);

    fn apply_operation(image: &mut Self, operation: &Self::Operation) {
        match operation {
            ImageOperation::Replace { image: replacement } => *image = replacement.clone(),
            ImageOperation::SetMetadata { metadata } => image.metadata = metadata.clone(),
        }
    }

    fn implicit_name(&self) -> Option<String> {
        let name = self.source_name.trim();
        (!name.is_empty()).then(|| name.to_owned())
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
