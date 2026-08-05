use super::{sample_bytes, Audio};

#[test]
fn audio_serialization_uses_base64() {
    let bytes = sample_bytes();
    let audio = Audio::new("sample.mp3", "audio/mpeg", bytes.clone()).unwrap();
    let json = serde_json::to_string(&audio).unwrap();

    assert!(!json.contains("[0,1,2,3"));
    assert!(json.len() < bytes.len() * 3);
}
