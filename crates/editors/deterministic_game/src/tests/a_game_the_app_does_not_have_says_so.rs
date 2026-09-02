use super::*;

#[test]
fn a_game_the_app_does_not_have_says_so() {
    let (mut editor, host) = editor("chess");

    answer(&host, AssetResult::Body(b"[]".to_vec()));
    editor.step();

    editor.find("game.error");
    editor.snapshot("a_game_the_app_does_not_have_says_so");
}
