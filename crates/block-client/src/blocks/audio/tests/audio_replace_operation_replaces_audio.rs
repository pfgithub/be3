use block::Block;

use super::{sample_bytes, Audio, AudioOperation};

#[test]
fn audio_replace_operation_replaces_audio() {
    let mut audio = Audio::new("before.mp3", "audio/mpeg", sample_bytes()).unwrap();
    let replacement = Audio::new("after.wav", "audio/wav", sample_bytes()).unwrap();

    Audio::apply_operation(
        &mut audio,
        &AudioOperation::Replace {
            audio: replacement.clone(),
        },
    );

    assert_eq!(audio, replacement);
}
