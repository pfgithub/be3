use super::*;

#[test]
fn a_recording_keeps_the_frames_it_was_given() {
    let recording = recorded(&["a label", "a longer label"]);
    let alone = captured(&["a longer label"]).pop().unwrap();

    assert_eq!(recording.frames.len(), 2);
    assert_eq!(recording.frames[1], alone.frames[0]);
    for (key, texture) in &alone.textures {
        assert_eq!(recording.textures.get(key), Some(texture));
    }
    assert!(recording.textures.len() < recording.frames.len() * alone.textures.len());

    let bytes = recording.encode().unwrap();
    assert_eq!(Snapshot::decode(&bytes).unwrap(), recording);
}
