use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PaintSnapshot {
    path: String,
    hash: String,
    #[serde(
        serialize_with = "serialize_data",
        deserialize_with = "deserialize_data"
    )]
    data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum PaintSnapshotOperation {
    Replace { snapshot: PaintSnapshot },
}

impl PaintSnapshot {
    pub const FILE_EXTENSION: &'static str = "paint";

    pub fn new(path: impl Into<String>, data: Vec<u8>) -> Self {
        let hash = Self::fingerprint(&data);
        Self {
            path: path.into(),
            hash,
            data,
        }
    }

    pub fn fingerprint(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .fold(String::new(), |mut hash, byte| {
                use std::fmt::Write as _;
                let _ = write!(hash, "{byte:02x}");
                hash
            })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Block for PaintSnapshot {
    type Operation = PaintSnapshotOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7061_696e_742d_736e_6170_7368_6f74_0001);

    fn apply_operation(snapshot: &mut Self, operation: &Self::Operation) {
        match operation {
            PaintSnapshotOperation::Replace {
                snapshot: replacement,
            } => *snapshot = replacement.clone(),
        }
    }

    fn implicit_name(&self) -> Option<String> {
        let name = self.path.rsplit('/').next().unwrap_or(&self.path).trim();
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
