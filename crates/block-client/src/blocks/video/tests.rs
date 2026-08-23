use std::path::PathBuf;

use block::BlockParent;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::*;
use crate::block_ref::WorktreeMembership;
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{BlockClient, ManagementClient};

mod video_attached_clips_start_at_their_offset;
mod video_attachment_cycles_are_refused;
mod video_base_clips_run_back_to_back;
mod video_clamps_clip_length_and_frame_rate;
mod video_clip_reference_is_excluded_and_resolves_to_the_target;
mod video_clip_reference_resolves_to_none_when_unresolvable;
mod video_history_undoes_and_redoes_a_rippling_removal;
mod video_history_undoes_and_redoes_trimming;
mod video_references_each_block_once;
mod video_removing_a_base_clip_ripples_and_takes_its_attachments;
mod video_serialization_round_trips;
mod video_visible_clips_are_ordered_from_the_base_up;

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root = std::env::temp_dir().join(format!("block-client-video-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server_root = root.clone();
        let handle = tokio::spawn(async move {
            block_server::serve(listener, server_root).await.unwrap();
        });
        Self { root, url, handle }
    }

    async fn shutdown(self) {
        self.handle.abort();
        let _ = self.handle.await;
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

async fn identity(url: &str) -> (Uuid, String, Uuid) {
    let management = ManagementClient::new(url).unwrap();
    let session = management
        .register(
            format!("{}@example.com", Uuid::new_v4()),
            "Test",
            "video-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}

                                                                            
                                                                             
fn sample() -> (Video, Uuid, Uuid, Uuid) {
    let mut video = Video::new();
    let first = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 10);
    let second = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 5);
    let attached = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 3).attached_to(first.id, 2);
    let ids = (first.id, second.id, attached.id);
    for (index, clip) in [first, second, attached].into_iter().enumerate() {
        Video::apply_operation(&mut video, &VideoOperation::InsertClip { clip, index });
    }
    (video, ids.0, ids.1, ids.2)
}

fn starts(video: &Video) -> Vec<(Uuid, u64, usize)> {
    video
        .timeline()
        .iter()
        .map(|timing| (timing.id, timing.start, timing.depth))
        .collect()
}
