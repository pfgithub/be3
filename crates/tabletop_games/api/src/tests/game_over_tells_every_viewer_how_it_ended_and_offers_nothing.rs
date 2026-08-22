use uuid::Uuid;

use super::GameHelper;

#[test]
fn game_over_tells_every_viewer_how_it_ended_and_offers_nothing() {
    let winner = Uuid::new_v4();
    let loser = Uuid::new_v4();
    let ending = |player| {
        GameHelper::new(&[], player)
            .game_over(|viewer| {
                if viewer == winner {
                    "You win!".to_owned()
                } else {
                    "You lose!".to_owned()
                }
            })
            .expect_err("a game that is over never goes on")
    };

    assert_eq!(ending(winner).description, "You win!");
    assert_eq!(ending(loser).description, "You lose!");
    assert!(ending(winner).actions.is_empty());
}
