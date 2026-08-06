use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Audio {
    source_name: String,
    media_type: String,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum AudioOperation {
    Replace { audio: Audio },
}

#[derive(Deserialize)]
struct AudioData {
    source_name: String,
    media_type: String,
    #[serde(deserialize_with = "deserialize_data")]
    data: Vec<u8>,
}

impl Audio {
    pub fn new(
        source_name: impl Into<String>,
        media_type: impl Into<String>,
        data: Vec<u8>,
    ) -> Result<Self, String> {
        if data.is_empty() {
            return Err("audio data must not be empty".into());
        }
        Ok(Self {
            source_name: source_name.into(),
            media_type: media_type.into(),
            data,
        })
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl<'de> Deserialize<'de> for Audio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = AudioData::deserialize(deserializer)?;
        Self::new(data.source_name, data.media_type, data.data).map_err(D::Error::custom)
    }
}

impl Block for Audio {
    type Operation = AudioOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6175_6469_6f2d_626c_6f63_6b2d_7479_7001);

    fn apply_operation(audio: &mut Self, operation: &Self::Operation) {
        match operation {
            AudioOperation::Replace { audio: replacement } => *audio = replacement.clone(),
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
