use super::Audio;

#[test]
fn audio_rejects_empty_data() {
    assert!(Audio::new("empty.mp3", "audio/mpeg", Vec::new()).is_err());
}
