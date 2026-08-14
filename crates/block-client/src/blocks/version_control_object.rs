use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, NoHistory};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A lookup key for an object's content, computed by hashing its serialized
/// payload (see [`VersionControlObject::content_hash`]). Not this block's
/// live id - a `version_control_data` repo keeps its own hash to object
/// index, built by walking its commit history, to decide whether a commit
/// can reuse an existing object instead of creating a new one.
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

/// Whether a tree entry names a leaf block's content or a nested container,
/// the same distinction git's tree entry mode makes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeEntryKind {
    Blob,
    Tree,
}

/// One member of a tree object: the permanent identity a worktree assigned it
/// (`eternal_id`, never derived from `name`, so a rename doesn't lose
/// identity), whether it names a blob or a nested tree, the object it
/// currently points at, and the name it is checked out under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TreeEntry {
    pub eternal_id: Uuid,
    pub kind: TreeEntryKind,
    pub content_hash: ObjectHash,
    pub name: String,
}

/// An object's content: either a blob, a snapshot of one block's serialized
/// state at commit time, or a tree, listing the objects nested under it.
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

/// A content-addressed object in a `version_control_data` repo's store (see
/// `external/vcs.md`): a blob or a tree, immutable once created - there is no
/// operation that mutates one, only creation with its final payload. Objects
/// get an ordinary, randomly-generated live id; `content_hash` is only a
/// lookup key; two commits racing on identical new content may each create an
/// object for the same hash, and the loser simply ends up unreferenced by any
/// tree.
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

    /// The lookup key a repo's hash to object index keys this object under.
    /// Derived fresh from the payload every time rather than cached on the
    /// block, so it can never disagree with the content it describes.
    /// Identical payloads (down to tree entry order, since a reordering
    /// checks out differently) always hash the same, which is what lets a
    /// commit dedup against existing objects by hash alone.
    pub fn content_hash(&self) -> ObjectHash {
        let bytes = serde_json::to_vec(&self.payload).expect("object payload always serializes");
        ObjectHash::of(&bytes)
    }
}

/// No operation ever mutates an object once created - its payload is fixed at
/// `create_block` time, matching "immutable once created" in
/// `external/vcs.md`. An uninhabited enum makes that a compile-time
/// guarantee rather than a convention every call site has to honor.
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
