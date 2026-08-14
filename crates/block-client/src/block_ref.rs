use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::BlockClient;

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockRef {
    Direct(Uuid),
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

pub trait WorktreeMembership: Send + Sync {
    fn worktree_type_id(&self) -> Uuid;

    fn repo_id(&self, client: &BlockClient, worktree_id: Uuid) -> Option<Uuid>;

    fn eternal_id_for_member(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        live_id: Uuid,
    ) -> Option<Uuid>;

    fn mint_eternal_id(&self, client: &BlockClient, worktree_id: Uuid, live_id: Uuid) -> Uuid;

    fn resolve_eternal_id(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        eternal_id: Uuid,
    ) -> Option<Uuid>;
}

#[cfg(test)]
mod tests;
