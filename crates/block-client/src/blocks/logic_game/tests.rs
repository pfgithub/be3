use std::path::PathBuf;

use block::BlockParent;
use logicgame::challenges::ChallengeId;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{LogicGame, LogicGameOperation, QuizRow};
use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::logic_grid::LogicGrid;
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::{BlockClient, BlockHandle, ManagementClient};

fn client_with_game() -> (BlockClient, BlockHandle<LogicGame>) {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(LogicGame::new());
    (client, block)
}

fn solutions(game: &LogicGame, challenge: ChallengeId) -> Vec<BlockRef> {
    game.level(challenge).unwrap().solutions.clone()
}

mod logic_game_history_restores_a_removed_solution_in_place;
mod logic_game_lists_every_built_in_challenge;
mod logic_game_records_quiz_answers_per_problem;
mod logic_game_references_its_solutions;
mod logic_game_solution_reference_is_excluded_and_resolves_to_the_target;
mod logic_game_solution_reference_resolves_to_none_when_unresolvable;

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root =
            std::env::temp_dir().join(format!("block-client-logic-game-test-{}", Uuid::new_v4()));
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
            "logic-game-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
