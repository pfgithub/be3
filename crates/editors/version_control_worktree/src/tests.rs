use std::sync::Arc;

use block_client::blocks::checklist::Checklist;
use block_client::blocks::version_control_data::{VersionControlData, MAIN_BRANCH};
use block_client::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeOperation,
};
use block_client::BlockClient;
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::VersionControlWorktreeApp;

mod every_member_of_the_worktree_gets_a_row;
mod the_checked_out_branch_is_marked_in_the_sidebar;

struct Fixture {
    editor: EditorTest<'static, VersionControlWorktreeApp>,
    members: Vec<Uuid>,
}

const ACCOUNT: Uuid = Uuid::from_u128(0x7663_732d_7465_7374_2d61_6363_6f75_6e74);
const WORKSPACE: Uuid = Uuid::from_u128(0x7663_732d_7465_7374_2d77_6f72_6b73_7061);

fn editor(members: usize) -> Fixture {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let data = client.create_block(VersionControlData::new(client.account_id(), 0));
    let worktree = client.create_block(VersionControlWorktree::new(
        data.id(),
        &data.read().expect("the repository was just created"),
    ));
    let members: Vec<Uuid> = (0..members)
        .map(|index| {
            let member = client.create_block(Checklist::default());
            worktree.operate(VersionControlWorktreeOperation::AddMember {
                live_id: member.id(),
                eternal_id: Uuid::from_u128(index as u128 + 1),
            });
            member.id()
        })
        .collect();

    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let mut app = VersionControlWorktreeApp::default();
    app.connect(host, client, worktree.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    Fixture { editor, members }
}
