use std::collections::BTreeMap;

use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::WorktreeMembership;
use crate::BlockClient;

use super::version_control_data::{CommitId, VersionControlData, MAIN_BRANCH};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VersionControlWorktree {
    repo: Uuid,
    checked_out_commit: CommitId,
    members_by_eternal_id: BTreeMap<Uuid, Uuid>,
    members_by_live_id: BTreeMap<Uuid, Uuid>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum VersionControlWorktreeOperation {
    AddMember { live_id: Uuid, eternal_id: Uuid },
    RemoveMember { live_id: Uuid },
    SetCheckedOutCommit { commit: CommitId },
}

impl VersionControlWorktree {
    pub fn new(repo: Uuid, data: &VersionControlData) -> Self {
        let checked_out_commit = data.branch_head(MAIN_BRANCH).cloned().unwrap_or_default();
        Self {
            repo,
            checked_out_commit,
            members_by_eternal_id: BTreeMap::new(),
            members_by_live_id: BTreeMap::new(),
        }
    }

    pub fn repo(&self) -> Uuid {
        self.repo
    }

    pub fn checked_out_commit(&self) -> &CommitId {
        &self.checked_out_commit
    }

    pub fn eternal_id_for_member(&self, live_id: Uuid) -> Option<Uuid> {
        self.members_by_live_id.get(&live_id).copied()
    }

    pub fn resolve_eternal_id(&self, eternal_id: Uuid) -> Option<Uuid> {
        self.members_by_eternal_id.get(&eternal_id).copied()
    }

    pub fn members(&self) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
        self.members_by_eternal_id
            .iter()
            .map(|(eternal_id, live_id)| (*eternal_id, *live_id))
    }
}

impl Block for VersionControlWorktree {
    type Operation = VersionControlWorktreeOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7663_732d_776f_726b_7472_6565_2d62_6c6b);

    fn references(&self) -> Vec<Uuid> {
        std::iter::once(self.repo)
            .chain(self.members_by_eternal_id.values().copied())
            .collect()
    }

    fn apply_operation(worktree: &mut Self, operation: &Self::Operation) {
        match operation {
            VersionControlWorktreeOperation::AddMember {
                live_id,
                eternal_id,
            } => {
                if worktree.members_by_live_id.contains_key(live_id)
                    || worktree.members_by_eternal_id.contains_key(eternal_id)
                {
                    return;
                }
                worktree.members_by_live_id.insert(*live_id, *eternal_id);
                worktree.members_by_eternal_id.insert(*eternal_id, *live_id);
            }
            VersionControlWorktreeOperation::RemoveMember { live_id } => {
                if let Some(eternal_id) = worktree.members_by_live_id.remove(live_id) {
                    worktree.members_by_eternal_id.remove(&eternal_id);
                }
            }
            VersionControlWorktreeOperation::SetCheckedOutCommit { commit } => {
                worktree.checked_out_commit = commit.clone();
            }
        }
    }

    fn add_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        if self.eternal_id_for_member(block_id).is_some() {
            return Some(Vec::new());
        }
        Some(vec![VersionControlWorktreeOperation::AddMember {
            live_id: block_id,
            eternal_id: Uuid::new_v4(),
        }])
    }

    fn delete_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        if self.eternal_id_for_member(block_id).is_none() {
            return Some(Vec::new());
        }
        Some(vec![VersionControlWorktreeOperation::RemoveMember {
            live_id: block_id,
        }])
    }
}

pub struct VersionControlWorktreeMembership;

impl WorktreeMembership for VersionControlWorktreeMembership {
    fn worktree_type_id(&self) -> Uuid {
        VersionControlWorktree::TYPE_ID
    }

    fn repo_id(&self, client: &BlockClient, worktree_id: Uuid) -> Option<Uuid> {
        Some(
            client
                .get_block::<VersionControlWorktree>(worktree_id)
                .read()?
                .repo(),
        )
    }

    fn eternal_id_for_member(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        live_id: Uuid,
    ) -> Option<Uuid> {
        client
            .get_block::<VersionControlWorktree>(worktree_id)
            .read()?
            .eternal_id_for_member(live_id)
    }

    fn mint_eternal_id(&self, client: &BlockClient, worktree_id: Uuid, live_id: Uuid) -> Uuid {
        let eternal_id = Uuid::new_v4();
        client
            .get_block::<VersionControlWorktree>(worktree_id)
            .operate(VersionControlWorktreeOperation::AddMember {
                live_id,
                eternal_id,
            });
        eternal_id
    }

    fn resolve_eternal_id(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        eternal_id: Uuid,
    ) -> Option<Uuid> {
        client
            .get_block::<VersionControlWorktree>(worktree_id)
            .read()?
            .resolve_eternal_id(eternal_id)
    }
}

#[cfg(test)]
mod tests;
