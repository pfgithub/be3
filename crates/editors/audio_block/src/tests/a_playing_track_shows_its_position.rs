use super::*;

#[test]
fn a_playing_track_shows_its_position() {
    let (mut editor, host, _block) = editor();

    host.set_audio(AudioStatus {
        playing: true,
        position_micros: 65_000_000,
        duration_micros: Some(200_000_000),
        error: None,
    });
    editor.step();

    editor.snapshot("a_playing_track_shows_its_position");
}
