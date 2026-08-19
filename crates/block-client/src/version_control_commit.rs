use std::collections::BTreeMap;

use block::{Block, BlockReferenceList};
use uuid::Uuid;

use crate::blocks::audio::Audio;
use crate::blocks::calendar::Calendar;
use crate::blocks::compiled_logic::CompiledLogic;
use crate::blocks::database::Database;
use crate::blocks::database_schema::DatabaseSchema;
use crate::blocks::database_view::DatabaseView;
use crate::blocks::gui_builder::GuiBuilder;
use crate::blocks::hotbar::Hotbar;
use crate::blocks::image::Image;
use crate::blocks::infinite_canvas::InfiniteCanvas;
use crate::blocks::logic_game::LogicGame;
use crate::blocks::logic_grid::LogicGrid;
use crate::blocks::map::Map;
use crate::blocks::pdf::Pdf;
use crate::blocks::pixel_art::PixelArt;
use crate::blocks::pixel_ray_tracer::PixelRayTracer;
use crate::blocks::presentation::Presentation;
use crate::blocks::settings::Settings;
use crate::blocks::text::TextDocument;
use crate::blocks::version_control_data::{
    Commit, CommitId, VersionControlData, VersionControlDataOperation, MAIN_BRANCH,
};
use crate::blocks::version_control_object::{
    ObjectHash, TreeEntry, TreeEntryKind, VersionControlObject,
};
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeOperation,
};
use crate::blocks::video::Video;
use crate::blocks::web_browser_tab::WebBrowserTab;
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{properties, BlockClient, BlockHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitOutcome {
    pub commit: CommitId,
    pub tree_hash: ObjectHash,
    pub branch_advanced: bool,
}

pub(crate) struct LiveEntry {
    pub eternal_id: Uuid,
    pub name: String,
    pub blob: VersionControlObject,
}

pub(crate) async fn live_blobs(
    client: &BlockClient,
    worktree_id: Uuid,
    members: &[(Uuid, Uuid)],
) -> Vec<LiveEntry> {
    let mut reference_by_id: BTreeMap<_, _> = client
        .list_references(BlockReferenceList::References(worktree_id))
        .await
        .into_iter()
        .map(|reference| (reference.id, reference))
        .collect();

    let mut result = Vec::with_capacity(members.len());
    for (eternal_id, live_id) in members {
        let Some(reference) = reference_by_id.remove(live_id) else {
            continue;
        };
        let Some(state) = snapshot_state(client, reference.block_type, *live_id).await else {
            continue;
        };
        let name = properties::read_name(&reference.properties)
            .map(|name| name.value)
            .unwrap_or_default();
        result.push(LiveEntry {
            eternal_id: *eternal_id,
            name,
            blob: VersionControlObject::blob(reference.block_type, state),
        });
    }
    result
}

pub async fn commit_worktree(
    client: &BlockClient,
    worktree_id: Uuid,
    author: Uuid,
    time: i64,
    message: String,
) -> Option<CommitOutcome> {
    let worktree = client.get_block::<VersionControlWorktree>(worktree_id);
    worktree.loaded().await;
    let (repo_id, parent, members) = {
        let state = worktree.read()?;
        (
            state.repo(),
            state.checked_out_commit().clone(),
            state.members().collect::<Vec<_>>(),
        )
    };

    let data = client.get_block::<VersionControlData>(repo_id);
    data.loaded().await;

    let live = live_blobs(client, worktree_id, &members).await;
    let mut entries = Vec::with_capacity(live.len());
    for entry in live {
        let hash = register_object(client, &data, entry.blob);
        entries.push(TreeEntry {
            eternal_id: entry.eternal_id,
            kind: TreeEntryKind::Blob,
            content_hash: hash,
            name: entry.name,
        });
    }

    let tree_hash = register_object(client, &data, VersionControlObject::tree(entries));

    let commit = Commit {
        parent: Some(parent.clone()),
        tree_hash: tree_hash.clone(),
        author,
        time,
        message: message.clone(),
    };
    let commit_id = commit.id();

    data.operate(VersionControlDataOperation::AppendCommit {
        parent: Some(parent.clone()),
        tree_hash: tree_hash.clone(),
        author,
        time,
        message,
    });
    data.operate(VersionControlDataOperation::SetBranch {
        name: MAIN_BRANCH.to_owned(),
        expected: Some(parent),
        commit: commit_id.clone(),
    });
    let branch_advanced = data
        .read()
        .is_some_and(|state| state.branch_head(MAIN_BRANCH) == Some(&commit_id));

    worktree.operate(VersionControlWorktreeOperation::SetCheckedOutCommit {
        commit: commit_id.clone(),
    });

    client.synchronized().await;

    Some(CommitOutcome {
        commit: commit_id,
        tree_hash,
        branch_advanced,
    })
}

pub(crate) fn register_object(
    client: &BlockClient,
    data: &BlockHandle<VersionControlData>,
    object: VersionControlObject,
) -> ObjectHash {
    let hash = object.content_hash();
    if data
        .read()
        .is_some_and(|state| state.contains_object(&hash))
    {
        return hash;
    }
    let created = client.create_block(object);
    data.operate(VersionControlDataOperation::RegisterObject {
        hash: hash.clone(),
        block: created.id(),
    });
    hash
}

async fn snapshot_state(client: &BlockClient, block_type: Uuid, id: Uuid) -> Option<Vec<u8>> {
    if !load_by_type(client, block_type, id).await {
        return None;
    }
    client.block_state_bytes(id)
}

macro_rules! for_each_source_block_type {
    ($apply:ident) => {
        $apply!(Audio);
        $apply!(Calendar);
        $apply!(CompiledLogic);
        $apply!(Database);
        $apply!(DatabaseSchema);
        $apply!(DatabaseView);
        $apply!(GuiBuilder);
        $apply!(Hotbar);
        $apply!(Image);
        $apply!(InfiniteCanvas);
        $apply!(LogicGame);
        $apply!(LogicGrid);
        $apply!(Map);
        $apply!(Pdf);
        $apply!(PixelArt);
        $apply!(PixelRayTracer);
        $apply!(Presentation);
        $apply!(Settings);
        $apply!(TextDocument);
        $apply!(Video);
        $apply!(WebBrowserTab);
        $apply!(WorkspaceIndex);
    };
}

async fn load_by_type(client: &BlockClient, block_type: Uuid, id: Uuid) -> bool {
    macro_rules! check {
        ($ty:ident) => {
            if block_type == $ty::TYPE_ID {
                client.get_block::<$ty>(id).loaded().await;
                return true;
            }
        };
    }
    for_each_source_block_type!(check);
    false
}

pub(crate) fn create_block_from_state(
    client: &BlockClient,
    block_type: Uuid,
    state: &[u8],
) -> Option<Uuid> {
    macro_rules! check {
        ($ty:ident) => {
            if block_type == $ty::TYPE_ID {
                let stored: crate::StoredBlock<$ty> = serde_json::from_slice(state).ok()?;
                return Some(client.create_block(stored.value).id());
            }
        };
    }
    for_each_source_block_type!(check);
    None
}

#[cfg(test)]
mod tests;
