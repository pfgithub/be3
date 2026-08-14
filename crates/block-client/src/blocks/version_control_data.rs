use std::collections::BTreeMap;

use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::version_control_object::{ObjectHash, VersionControlObject};

/// The branch every `version_control_data` repo starts with.
pub const MAIN_BRANCH: &str = "main";

/// A lookup key for a [`Commit`], the sha256 of its own serialized fields
/// (excluding this id, which is derived from them). Content-addressed like
/// [`ObjectHash`] rather than a random id, so two clients that independently
/// build byte-identical commits (same parent, tree, author, time, message)
/// agree on its id without coordinating.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CommitId(ObjectHash);

impl CommitId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One node in a repo's commit history: the tree it snapshots, who made it
/// and when, and (except for a repo's very first commit) the commit it
/// follows. Once appended, a commit is never mutated - the DAG only grows.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Commit {
    pub parent: Option<CommitId>,
    pub tree_hash: ObjectHash,
    pub author: Uuid,
    pub time: i64,
    pub message: String,
}

impl Commit {
    /// This commit's [`CommitId`], derived fresh from its fields every time
    /// so it can never disagree with the content it describes, the same
    /// reasoning as [`VersionControlObject::content_hash`].
    pub fn id(&self) -> CommitId {
        let bytes = serde_json::to_vec(self).expect("commit always serializes");
        CommitId(ObjectHash::of(&bytes))
    }
}

/// A repo: a branch name to commit map plus the append-only commit history
/// those branches point into, and this repo's own hash to live object id
/// index (see [`VersionControlData::object_id`] for why that index has to
/// live here rather than being derived on read).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VersionControlData {
    branches: BTreeMap<String, CommitId>,
    commits: BTreeMap<CommitId, Commit>,
    objects: BTreeMap<ObjectHash, Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum VersionControlDataOperation {
    /// Adds a commit to the history. Pure and additive: since a `Commit`'s
    /// id is content-addressed, appending one that already exists (e.g. two
    /// clients racing to build the same commit) is a harmless no-op rather
    /// than a duplicate. Ignored if `parent` is set but not already in the
    /// history, which would otherwise leave a dangling ancestry link.
    AppendCommit {
        parent: Option<CommitId>,
        tree_hash: ObjectHash,
        author: Uuid,
        time: i64,
        message: String,
    },
    /// Records the live block id backing a `version_control_object` this
    /// repo created for `hash`. Ignored if `hash` is already registered -
    /// first writer wins, so if two commits race on identical new content
    /// (see `external/vcs.md`) the loser's object is simply never pointed at
    /// by this index and sits orphaned, exactly as the design doc describes.
    RegisterObject { hash: ObjectHash, block: Uuid },
    /// Creates or advances a branch, compare-and-swap style: applied only if
    /// `name` currently points at `expected` (`None` meaning the branch must
    /// not exist yet, for branch creation). This is the one genuinely
    /// contested operation in the model - a caller that loses the race reads
    /// the branch's new head and reconciles by hand (e.g. by creating a new
    /// branch instead). Also ignored if `commit` is not in the history, so a
    /// branch can never point outside the commit DAG.
    SetBranch {
        name: String,
        expected: Option<CommitId>,
        commit: CommitId,
    },
}

impl VersionControlData {
    /// A fresh repo: `main` pointing at an empty initial commit. The initial
    /// commit's tree hash is the hash an empty `VersionControlObject` tree
    /// would have, computed the same way any other tree's hash is - but no
    /// such object is actually created here. Hashing a payload is a pure
    /// function of its content, so the hash is well-defined and usable for
    /// comparisons (e.g. "does the worktree's live tree match what's checked
    /// out") whether or not any block with that content exists yet; a real
    /// object only needs to be created the first time something is actually
    /// entered into the tree, same as any other reused hash.
    pub fn new(author: Uuid, time: i64) -> Self {
        let initial = Commit {
            parent: None,
            tree_hash: empty_tree_hash(),
            author,
            time,
            message: "Initial commit".to_owned(),
        };
        let id = initial.id();
        let mut commits = BTreeMap::new();
        commits.insert(id.clone(), initial);
        let mut branches = BTreeMap::new();
        branches.insert(MAIN_BRANCH.to_owned(), id);
        Self {
            branches,
            commits,
            objects: BTreeMap::new(),
        }
    }

    pub fn branches(&self) -> &BTreeMap<String, CommitId> {
        &self.branches
    }

    pub fn branch_head(&self, name: &str) -> Option<&CommitId> {
        self.branches.get(name)
    }

    pub fn commits(&self) -> &BTreeMap<CommitId, Commit> {
        &self.commits
    }

    pub fn commit(&self, id: &CommitId) -> Option<&Commit> {
        self.commits.get(id)
    }

    /// `from` and every commit reachable by following `parent`, nearest
    /// first. Stops at the first id missing from the history rather than
    /// panicking, so a caller can't be handed a partial chain silently
    /// mistaken for the whole thing - an empty result past the first entry
    /// means the DAG was walked as far as this repo's own state allows.
    pub fn ancestors(&self, from: &CommitId) -> Vec<CommitId> {
        let mut chain = Vec::new();
        let mut current = Some(from.clone());
        while let Some(id) = current {
            let Some(commit) = self.commits.get(&id) else {
                break;
            };
            current = commit.parent.clone();
            chain.push(id);
        }
        chain
    }

    /// The live id of the `version_control_object` this repo already created
    /// for `hash`, if any. A tree entry only ever stores a
    /// [`ObjectHash`] for its target (never a live id), so resolving one to
    /// an actual block - including deciding whether a commit needs to create
    /// a new object at all - depends on this synced index rather than being
    /// derivable by re-hashing something already in hand. It is maintained
    /// incrementally via [`VersionControlDataOperation::RegisterObject`]
    /// rather than rebuilt by walking the commit DAG, since a commit's own
    /// record only holds its root tree hash, not the hashes of everything
    /// nested under it.
    pub fn object_id(&self, hash: &ObjectHash) -> Option<Uuid> {
        self.objects.get(hash).copied()
    }

    pub fn contains_object(&self, hash: &ObjectHash) -> bool {
        self.objects.contains_key(hash)
    }
}

/// The content hash an empty `VersionControlObject` tree would have. Shared
/// by [`VersionControlData::new`] and anything else that needs to recognize
/// or compare against "no content" without creating an object for it.
pub fn empty_tree_hash() -> ObjectHash {
    VersionControlObject::tree(Vec::new()).content_hash()
}

impl Block for VersionControlData {
    type Operation = VersionControlDataOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7663_732d_6461_7461_2d62_6c6f_636b_3030);

    /// The live `version_control_object` blocks this repo's hash index
    /// currently points at. A losing object from a create race (see
    /// [`VersionControlDataOperation::RegisterObject`]) never enters
    /// `objects`, so it never appears here either - it ends up with no
    /// backref, exactly the "sits orphaned" outcome `external/vcs.md`
    /// describes.
    fn references(&self) -> Vec<Uuid> {
        self.objects.values().copied().collect()
    }

    fn apply_operation(data: &mut Self, operation: &Self::Operation) {
        match operation {
            VersionControlDataOperation::AppendCommit {
                parent,
                tree_hash,
                author,
                time,
                message,
            } => {
                if let Some(parent_id) = parent {
                    if !data.commits.contains_key(parent_id) {
                        return;
                    }
                }
                let commit = Commit {
                    parent: parent.clone(),
                    tree_hash: tree_hash.clone(),
                    author: *author,
                    time: *time,
                    message: message.clone(),
                };
                data.commits.entry(commit.id()).or_insert(commit);
            }
            VersionControlDataOperation::RegisterObject { hash, block } => {
                data.objects.entry(hash.clone()).or_insert(*block);
            }
            VersionControlDataOperation::SetBranch {
                name,
                expected,
                commit,
            } => {
                if !data.commits.contains_key(commit) {
                    return;
                }
                if data.branches.get(name) != expected.as_ref() {
                    return;
                }
                data.branches.insert(name.clone(), commit.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests;
