use std::collections::BTreeMap;

use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::WorktreeMembership;
use crate::BlockClient;

use super::version_control_data::{CommitId, VersionControlData, MAIN_BRANCH};

/// A worktree: a checkout of a `version_control_data` repo, plus this
/// worktree's own permanent `eternal_id` for each live block that is
/// currently a member of it (see `external/vcs.md`). Content itself lives as
/// ordinary child blocks under this one via normal `BlockParent`, not in this
/// state - the map here only exists so a reference elsewhere in the same
/// worktree can survive a rename or a checkout that swaps the live id out
/// from under it.
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
    /// Records `eternal_id` as the permanent identity of `live_id`, which is
    /// becoming a member of this worktree for the first time - either just
    /// created inside it or dragged in from elsewhere. Ignored if `live_id`
    /// is already a member (first writer wins, same reasoning as
    /// `VersionControlDataOperation::RegisterObject`) or if `eternal_id` is
    /// already in use by another member.
    AddMember { live_id: Uuid, eternal_id: Uuid },
    /// Drops `live_id`'s membership, freeing its `eternal_id`. A no-op if
    /// `live_id` isn't currently a member.
    RemoveMember { live_id: Uuid },
    /// Updates the commit this worktree is checked out at. Unconditional -
    /// the dirty-check and discard-confirmation that must gate a real
    /// checkout belong to the caller (see unit 6 of `external/vcs.md`), not
    /// to this block's own state.
    SetCheckedOutCommit { commit: CommitId },
}

impl VersionControlWorktree {
    /// A fresh worktree over `repo`, checked out at `data`'s `main` branch
    /// head and starting with no members - creating one against a fresh
    /// `VersionControlData` block means `data` is still just its own initial
    /// empty-tree commit. Any starter content (e.g. a `WorkspaceIndex` folder
    /// to hold it) is created by the caller afterward, the same way every
    /// other multi-block bootstrap in this codebase works (see
    /// `editors/database.rs` creating a schema, then a database, then a
    /// view) rather than cascading out of a block constructor, which has no
    /// access to a `BlockClient` to create anything with.
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

    /// The repo this worktree checks out, plus every live block currently
    /// backing one of its members - kept up to date the same way
    /// `WorkspaceIndex::references` tracks its entries, even though most
    /// members' actual `BlockParent` points at a container nested somewhere
    /// under this worktree rather than at this block directly.
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
}

/// Plugs [`VersionControlWorktree`] into
/// [`BlockClient::classify_reference`]/[`BlockClient::resolve_reference`],
/// the seam `WorktreeMembership` exists for (see `crate::block_ref`). Holds
/// no state of its own - every method reads or mutates the worktree block
/// named by `worktree_id` through `client`.
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
