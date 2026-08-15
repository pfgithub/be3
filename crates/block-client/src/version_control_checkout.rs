use std::collections::BTreeMap;

use block::BlockParent;
use uuid::Uuid;

use crate::blocks::version_control_data::{empty_tree_hash, CommitId, VersionControlData};
use crate::blocks::version_control_object::{
    ObjectHash, ObjectPayload, TreeEntry, TreeEntryKind, VersionControlObject,
};
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeOperation,
};
use crate::version_control_commit::{create_block_from_state, live_blobs};
use crate::{BlockClient, BlockHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckoutOutcome {
    Blocked,
    Applied {
        created: Vec<Uuid>,
        replaced: Vec<Uuid>,
        detached: Vec<Uuid>,
    },
}

pub async fn worktree_is_clean(client: &BlockClient, worktree_id: Uuid) -> Option<bool> {
    let worktree = client.get_block::<VersionControlWorktree>(worktree_id);
    worktree.loaded().await;
    let (repo_id, checked_out, members) = {
        let state = worktree.read()?;
        (
            state.repo(),
            state.checked_out_commit().clone(),
            state.members().collect::<Vec<_>>(),
        )
    };

    let data = client.get_block::<VersionControlData>(repo_id);
    data.loaded().await;
    let checked_out_hash = data.read()?.commit(&checked_out)?.tree_hash.clone();
    let live_hash = live_tree_hash(client, worktree_id, &members).await;
    Some(live_hash == checked_out_hash)
}

pub async fn checkout_worktree(
    client: &BlockClient,
    worktree_id: Uuid,
    target: CommitId,
    discard: bool,
) -> Option<CheckoutOutcome> {
    let worktree = client.get_block::<VersionControlWorktree>(worktree_id);
    worktree.loaded().await;
    let (repo_id, checked_out, members) = {
        let state = worktree.read()?;
        (
            state.repo(),
            state.checked_out_commit().clone(),
            state.members().collect::<Vec<_>>(),
        )
    };

    let data = client.get_block::<VersionControlData>(repo_id);
    data.loaded().await;

    let current_entries = live_tree_entries(client, worktree_id, &members).await;

    if !discard {
        let checked_out_hash = data.read()?.commit(&checked_out)?.tree_hash.clone();
        let live_hash = VersionControlObject::tree(current_entries.clone()).content_hash();
        if live_hash != checked_out_hash {
            return Some(CheckoutOutcome::Blocked);
        }
    }

    let target_tree_hash = data.read()?.commit(&target)?.tree_hash.clone();
    let target_entries = load_tree_entries(client, &data, &target_tree_hash).await?;

    let current_hash_by_eternal: BTreeMap<Uuid, ObjectHash> = current_entries
        .into_iter()
        .map(|entry| (entry.eternal_id, entry.content_hash))
        .collect();
    let current_live_by_eternal: BTreeMap<Uuid, Uuid> = members.into_iter().collect();
    let target_by_eternal: BTreeMap<Uuid, &TreeEntry> = target_entries
        .iter()
        .map(|entry| (entry.eternal_id, entry))
        .collect();

    let mut created = Vec::new();
    let mut replaced = Vec::new();
    let mut detached = Vec::new();

    for entry in &target_entries {
        let current_live = current_live_by_eternal.get(&entry.eternal_id).copied();
        let unchanged = current_hash_by_eternal
            .get(&entry.eternal_id)
            .is_some_and(|hash| *hash == entry.content_hash);
        if unchanged {
            continue;
        }

        if let Some(live_id) = current_live {
            client.set_block_parent(live_id, BlockParent::Orphaned);
            worktree.operate(VersionControlWorktreeOperation::RemoveMember { live_id });
        }

        let Some(new_id) = create_from_entry(client, &data, entry).await else {
            continue;
        };
        worktree.operate(VersionControlWorktreeOperation::AddMember {
            live_id: new_id,
            eternal_id: entry.eternal_id,
        });
        client.set_block_parent(new_id, BlockParent::Uuid(worktree_id));
        client.set_block_name(new_id, entry.name.clone());

        if current_live.is_some() {
            replaced.push(new_id);
        } else {
            created.push(new_id);
        }
    }

    for (eternal_id, live_id) in current_live_by_eternal {
        if !target_by_eternal.contains_key(&eternal_id) {
            client.set_block_parent(live_id, BlockParent::Orphaned);
            worktree.operate(VersionControlWorktreeOperation::RemoveMember { live_id });
            detached.push(live_id);
        }
    }

    worktree.operate(VersionControlWorktreeOperation::SetCheckedOutCommit { commit: target });

    client.synchronized().await;

    Some(CheckoutOutcome::Applied {
        created,
        replaced,
        detached,
    })
}

pub async fn materialize_worktree(
    client: &BlockClient,
    worktree_id: Uuid,
) -> Option<CheckoutOutcome> {
    let worktree = client.get_block::<VersionControlWorktree>(worktree_id);
    worktree.loaded().await;
    let target = worktree.read()?.checked_out_commit().clone();
    checkout_worktree(client, worktree_id, target, true).await
}

async fn live_tree_hash(
    client: &BlockClient,
    worktree_id: Uuid,
    members: &[(Uuid, Uuid)],
) -> ObjectHash {
    VersionControlObject::tree(live_tree_entries(client, worktree_id, members).await).content_hash()
}

async fn live_tree_entries(
    client: &BlockClient,
    worktree_id: Uuid,
    members: &[(Uuid, Uuid)],
) -> Vec<TreeEntry> {
    live_blobs(client, worktree_id, members)
        .await
        .into_iter()
        .map(|entry| TreeEntry {
            eternal_id: entry.eternal_id,
            kind: TreeEntryKind::Blob,
            content_hash: entry.blob.content_hash(),
            name: entry.name,
        })
        .collect()
}

async fn load_tree_entries(
    client: &BlockClient,
    data: &BlockHandle<VersionControlData>,
    tree_hash: &ObjectHash,
) -> Option<Vec<TreeEntry>> {
    let object_id = data.read()?.object_id(tree_hash);
    let Some(object_id) = object_id else {
        return (*tree_hash == empty_tree_hash()).then(Vec::<TreeEntry>::new);
    };
    let object = client.get_block::<VersionControlObject>(object_id);
    object.loaded().await;
    let read = object.read()?;
    match read.payload() {
        ObjectPayload::Tree { entries } => Some(entries.clone()),
        ObjectPayload::Blob { .. } => None,
    }
}

async fn create_from_entry(
    client: &BlockClient,
    data: &BlockHandle<VersionControlData>,
    entry: &TreeEntry,
) -> Option<Uuid> {
    let object_id = data.read()?.object_id(&entry.content_hash)?;
    let object = client.get_block::<VersionControlObject>(object_id);
    object.loaded().await;
    let (source_block_type, state) = {
        let read = object.read()?;
        match read.payload() {
            ObjectPayload::Blob {
                source_block_type,
                state,
            } => (*source_block_type, state.clone()),
            ObjectPayload::Tree { .. } => return None,
        }
    };
    create_block_from_state(client, source_block_type, &state)
}

#[cfg(test)]
mod tests;
