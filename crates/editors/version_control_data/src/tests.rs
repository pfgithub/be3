use std::sync::Arc;

use block_client::blocks::version_control_data::{Commit, CommitId, VersionControlData};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::{format_commit_time, short_author, VersionControlDataApp};

mod creating_a_branch_points_it_at_the_selected_head;
mod format_commit_time_formats_readable_utc_string;
mod short_author_truncates_uuid_to_short_id_len;
mod short_commit_id_truncates_to_short_id_len;

fn editor() -> (
    EditorTest<'static, VersionControlDataApp>,
    BlockHandle<VersionControlData>,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(VersionControlData::new(Uuid::from_u128(1), 0));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = VersionControlDataApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
