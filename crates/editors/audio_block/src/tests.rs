use std::sync::Arc;

use block_client::blocks::audio::Audio;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, AudioStatus, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::{guess_media_type, AudioApp};

mod a_playing_track_shows_its_position;
mod the_media_type_follows_the_file_name;

fn editor() -> (
    EditorTest<'static, AudioApp>,
    EditorHost,
    BlockHandle<Audio>,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Audio::new("song.flac", "audio/flac", vec![1, 2, 3]).unwrap());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = AudioApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, host, block)
}
