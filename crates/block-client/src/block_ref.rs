use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::BlockClient;

/// How a block refers to another block, once the referencing block
/// participates in a repo (see `external/vcs.md`). A reference-bearing block
/// type (`text`, `database`, ...) stores these in place of a bare [`Uuid`].
#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockRef {
    /// A permanent live block id. Behaves exactly as an ordinary reference
    /// does today: included in `references()`, validated by the server.
    Direct(Uuid),
    /// The target is a member of the same worktree as the block holding this
    /// reference. Never rewritten by any commit or checkout, which is what
    /// keeps two worktrees checked out at the same commit byte-identical.
    /// Excluded from `references()`; resolved to a live id only at point of
    /// use via [`BlockClient::resolve_reference`].
    RepoRelative { repo: Uuid, eternal_id: Uuid },
}

impl BlockRef {
    pub fn as_direct(&self) -> Option<Uuid> {
        match self {
            Self::Direct(id) => Some(*id),
            Self::RepoRelative { .. } => None,
        }
    }
}

/// Plugs a concrete `version_control_worktree` block type into
/// [`BlockClient::classify_reference`] and [`BlockClient::resolve_reference`]
/// without either of them depending on it. No worktree block type exists yet
/// (it is unit 4 of `external/vcs.md`'s suggested units of work); this trait
/// is the seam that block type implements once it does, so callers pass an
/// implementation of it in rather than the helpers hard-coding one.
pub trait WorktreeMembership: Send + Sync {
    /// The worktree block type's `Block::TYPE_ID`.
    fn worktree_type_id(&self) -> Uuid;

    /// The `version_control_data` block `worktree_id` checks out from, used
    /// to fill in and later check a [`BlockRef::RepoRelative`]'s `repo` field.
    fn repo_id(&self, client: &BlockClient, worktree_id: Uuid) -> Option<Uuid>;

    /// The eternal id already assigned to `live_id` inside `worktree_id`, if
    /// it is already a member.
    fn eternal_id_for_member(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        live_id: Uuid,
    ) -> Option<Uuid>;

    /// Assigns a fresh eternal id to `live_id`, which is becoming a member of
    /// `worktree_id` for the first time.
    fn mint_eternal_id(&self, client: &BlockClient, worktree_id: Uuid, live_id: Uuid) -> Uuid;

    /// The live id currently backing `eternal_id` inside `worktree_id`, if it
    /// resolves to a current member.
    fn resolve_eternal_id(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        eternal_id: Uuid,
    ) -> Option<Uuid>;
}

#[cfg(test)]
mod tests;
