use super::*;

#[test]
fn expanding_a_level_shows_its_goal() {
    let (mut editor, block) = editor();

    let first = block.read().unwrap().levels()[0].challenge;
    editor
        .find(&format!("logic-game.level.{}", first as usize))
        .click();
    editor.run();

    assert!(editor
        .find(&format!("logic-game.new-attempt.{}", first as usize))
        .rect()
        .is_positive());
    editor.snapshot("expanding_a_level_shows_its_goal");
}
