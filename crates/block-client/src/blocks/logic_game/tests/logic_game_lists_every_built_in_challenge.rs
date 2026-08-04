use logicgame::challenges::CHALLENGES;

use super::*;

#[test]
fn logic_game_lists_every_built_in_challenge() {
    let game = LogicGame::new();

    assert_eq!(
        game.levels()
            .iter()
            .map(|level| level.challenge)
            .collect::<Vec<_>>(),
        CHALLENGES.to_vec()
    );
    assert!(game
        .levels()
        .iter()
        .all(|level| level.solutions.is_empty() && !level.completed));
}
