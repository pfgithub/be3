use std::sync::Arc;

use block_client::blocks::video::{Video, VideoFrameRate};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::VideoApp;
use crate::timeline::timecode;

mod an_empty_video_has_nothing_at_the_playhead;
mod timecode_counts_minutes_seconds_and_frames;

fn editor() -> (EditorTest<'static, VideoApp>, BlockHandle<Video>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Video::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = VideoApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
