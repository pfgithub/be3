use super::{sample_bytes, Audio};

#[test]
fn audio_serialization_preserves_data() {
    let bytes = sample_bytes();
    let audio = Audio::new("sample.mp3", "audio/mpeg", bytes.clone()).unwrap();

    let encoded = serde_json::to_vec(&audio).unwrap();
    let decoded: Audio = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, audio);
    assert_eq!(decoded.data(), bytes);
}
