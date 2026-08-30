use super::*;

#[test]
fn the_media_type_follows_the_file_name() {
    assert_eq!(guess_media_type("song.wav"), "audio/wav");
    assert_eq!(guess_media_type("song.OGA"), "audio/ogg");
    assert_eq!(guess_media_type("song.m4a"), "audio/mp4");
    assert_eq!(guess_media_type("song"), "audio/mpeg");
}
