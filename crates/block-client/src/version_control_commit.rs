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

    let mut reference_by_id: BTreeMap<_, _> = client
        .list_references(BlockReferenceList::References(worktree_id))
        .await
        .into_iter()
        .map(|reference| (reference.id, reference))
        .collect();

    let mut entries = Vec::with_capacity(members.len());
    for (eternal_id, live_id) in members {
        let Some(reference) = reference_by_id.remove(&live_id) else {
            continue;
        };
        let Some(state) = snapshot_state(client, reference.block_type, live_id).await else {
            continue;
        };
        let name = properties::read_name(&reference.properties)
            .map(|name| name.value)
            .unwrap_or_default();
        let hash = register_object(
            client,
            &data,
            VersionControlObject::blob(reference.block_type, state),
        );
        entries.push(TreeEntry {
            eternal_id,
            kind: TreeEntryKind::Blob,
            content_hash: hash,
            name,
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

    Some(CommitOutcome {
        commit: commit_id,
        tree_hash,
        branch_advanced,
    })
}

fn register_object(
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

async fn load_by_type(client: &BlockClient, block_type: Uuid, id: Uuid) -> bool {
    if block_type == Audio::TYPE_ID {
        client.get_block::<Audio>(id).loaded().await;
        return true;
    }
    if block_type == Calendar::TYPE_ID {
        client.get_block::<Calendar>(id).loaded().await;
        return true;
    }
    if block_type == CompiledLogic::TYPE_ID {
        client.get_block::<CompiledLogic>(id).loaded().await;
        return true;
    }
    if block_type == Database::TYPE_ID {
        client.get_block::<Database>(id).loaded().await;
        return true;
    }
    if block_type == DatabaseSchema::TYPE_ID {
        client.get_block::<DatabaseSchema>(id).loaded().await;
        return true;
    }
    if block_type == DatabaseView::TYPE_ID {
        client.get_block::<DatabaseView>(id).loaded().await;
        return true;
    }
    if block_type == GuiBuilder::TYPE_ID {
        client.get_block::<GuiBuilder>(id).loaded().await;
        return true;
    }
    if block_type == Hotbar::TYPE_ID {
        client.get_block::<Hotbar>(id).loaded().await;
        return true;
    }
    if block_type == Image::TYPE_ID {
        client.get_block::<Image>(id).loaded().await;
        return true;
    }
    if block_type == InfiniteCanvas::TYPE_ID {
        client.get_block::<InfiniteCanvas>(id).loaded().await;
        return true;
    }
    if block_type == LogicGame::TYPE_ID {
        client.get_block::<LogicGame>(id).loaded().await;
        return true;
    }
    if block_type == LogicGrid::TYPE_ID {
        client.get_block::<LogicGrid>(id).loaded().await;
        return true;
    }
    if block_type == Map::TYPE_ID {
        client.get_block::<Map>(id).loaded().await;
        return true;
    }
    if block_type == PixelArt::TYPE_ID {
        client.get_block::<PixelArt>(id).loaded().await;
        return true;
    }
    if block_type == PixelRayTracer::TYPE_ID {
        client.get_block::<PixelRayTracer>(id).loaded().await;
        return true;
    }
    if block_type == Presentation::TYPE_ID {
        client.get_block::<Presentation>(id).loaded().await;
        return true;
    }
    if block_type == Settings::TYPE_ID {
        client.get_block::<Settings>(id).loaded().await;
        return true;
    }
    if block_type == TextDocument::TYPE_ID {
        client.get_block::<TextDocument>(id).loaded().await;
        return true;
    }
    if block_type == Video::TYPE_ID {
        client.get_block::<Video>(id).loaded().await;
        return true;
    }
    if block_type == WebBrowserTab::TYPE_ID {
        client.get_block::<WebBrowserTab>(id).loaded().await;
        return true;
    }
    if block_type == WorkspaceIndex::TYPE_ID {
        client.get_block::<WorkspaceIndex>(id).loaded().await;
        return true;
    }
    false
}

#[cfg(test)]
mod tests;
