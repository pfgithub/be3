use uuid::Uuid;

use super::join_action;
use crate::{crazy_8s::Crazy8s, Game, GameAction};

#[test]
fn playing_greedily_from_both_sides_eventually_ends_the_game() {
    let p0 = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let game = Crazy8s;
    let mut actions = vec![join_action(p0), join_action(p1)];

    for _ in 0..500 {
        let screen0 = game.show(&actions, p0);
        let screen1 = game.show(&actions, p1);

        if screen0.actions.is_empty() && screen1.actions.is_empty() {
            let valid = matches!(
                (screen0.description.as_str(), screen1.description.as_str()),
                ("You win!", "You lose!")
                    | ("You lose!", "You win!")
                    | (
                        "Draw! Neither player can play.",
                        "Draw! Neither player can play."
                    )
            );
            assert!(
                valid,
                "unexpected ending: {:?} / {:?}",
                screen0.description, screen1.description
            );
            return;
        }

        if let Some(option) = screen0.actions.first() {
            actions.push(GameAction {
                actor: p0,
                action: option.effect.clone(),
            });
        } else if let Some(option) = screen1.actions.first() {
            actions.push(GameAction {
                actor: p1,
                action: option.effect.clone(),
            });
        }
    }

    panic!("game did not terminate within 500 actions");
}
