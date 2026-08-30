use std::collections::BTreeMap;

use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::version_control_object::{ObjectHash, VersionControlObject};

pub const MAIN_BRANCH: &str = "main";

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CommitId(ObjectHash);

impl CommitId {
    pub const SHORT_LEN: usize = 8;

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn short(&self) -> String {
        self.as_str().chars().take(Self::SHORT_LEN).collect()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Commit {
    pub parent: Option<CommitId>,
    pub tree_hash: ObjectHash,
    pub author: Uuid,
    pub time: i64,
    pub message: String,
}

impl Commit {
    pub fn id(&self) -> CommitId {
        let bytes = serde_json::to_vec(self).expect("commit always serializes");
        CommitId(ObjectHash::of(&bytes))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VersionControlData {
    branches: BTreeMap<String, CommitId>,
    commits: BTreeMap<CommitId, Commit>,
    objects: BTreeMap<ObjectHash, Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum VersionControlDataOperation {
    AppendCommit {
        parent: Option<CommitId>,
        tree_hash: ObjectHash,
        author: Uuid,
        time: i64,
        message: String,
    },
    RegisterObject {
        hash: ObjectHash,
        block: Uuid,
    },
    SetBranch {
        name: String,
        expected: Option<CommitId>,
        commit: CommitId,
    },
}

impl VersionControlData {
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

    pub fn object_id(&self, hash: &ObjectHash) -> Option<Uuid> {
        self.objects.get(hash).copied()
    }

    pub fn contains_object(&self, hash: &ObjectHash) -> bool {
        self.objects.contains_key(hash)
    }
}

pub fn empty_tree_hash() -> ObjectHash {
    VersionControlObject::tree(Vec::new()).content_hash()
}

impl Block for VersionControlData {
    type Operation = VersionControlDataOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7663_732d_6461_7461_2d62_6c6f_636b_3030);

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
