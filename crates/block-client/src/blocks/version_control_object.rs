use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ObjectHash(String);

impl ObjectHash {
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeEntryKind {
    Blob,
    Tree,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TreeEntry {
    pub eternal_id: Uuid,
    pub kind: TreeEntryKind,
    pub content_hash: ObjectHash,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObjectPayload {
    Blob {
        source_block_type: Uuid,
        #[serde(
            serialize_with = "serialize_state",
            deserialize_with = "deserialize_state"
        )]
        state: Vec<u8>,
    },
    Tree {
        entries: Vec<TreeEntry>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionControlObject {
    payload: ObjectPayload,
}

impl VersionControlObject {
    pub fn blob(source_block_type: Uuid, state: Vec<u8>) -> Self {
        Self {
            payload: ObjectPayload::Blob {
                source_block_type,
                state,
            },
        }
    }

    pub fn tree(entries: Vec<TreeEntry>) -> Self {
        Self {
            payload: ObjectPayload::Tree { entries },
        }
    }

    pub fn payload(&self) -> &ObjectPayload {
        &self.payload
    }

    pub fn content_hash(&self) -> ObjectHash {
        let bytes = serde_json::to_vec(&self.payload).expect("object payload always serializes");
        ObjectHash::of(&bytes)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VersionControlObjectOperation {}

impl Block for VersionControlObject {
    type Operation = VersionControlObjectOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7663_732d_6f62_6a65_6374_2d62_6c6f_636b);
    const CRDT: bool = true;

    fn apply_operation(_object: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

fn serialize_state<S>(state: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(state))
}

fn deserialize_state<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests;
