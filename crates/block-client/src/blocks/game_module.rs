use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameModule {
    source_name: String,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GameModuleOperation {
    Replace { module: GameModule },
}

impl GameModule {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &["wasm"];
    pub const MIME_TYPES: &'static [&'static str] = &["application/wasm"];

    pub fn new(source_name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            source_name: source_name.into(),
            data,
        }
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Block for GameModule {
    type Operation = GameModuleOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6761_6d65_2d6d_6f64_756c_652d_626c_0001);

    fn apply_operation(module: &mut Self, operation: &Self::Operation) {
        match operation {
            GameModuleOperation::Replace {
                module: replacement,
            } => *module = replacement.clone(),
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
