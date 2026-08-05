use block::Block;

use super::{sample_bytes, Audio};

#[test]
fn audio_implicit_name_uses_source_name() {
    let named = Audio::new("song.mp3", "audio/mpeg", sample_bytes()).unwrap();
    let unnamed = Audio::new("  ", "audio/mpeg", sample_bytes()).unwrap();

    assert_eq!(named.implicit_name(), "song.mp3");
    assert_eq!(unnamed.implicit_name(), "Audio");
}
