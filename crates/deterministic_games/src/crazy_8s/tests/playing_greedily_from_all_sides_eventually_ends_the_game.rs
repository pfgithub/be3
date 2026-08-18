use uuid::Uuid;

use super::{join, start};
use crate::{crazy_8s::Crazy8s, Game, GameAction};

#[test]
fn playing_greedily_from_all_sides_eventually_ends_the_game() {
    let players: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let game = Crazy8s;
    let mut actions: Vec<GameAction> = Vec::new();
    for player in players.iter().copied() {
        let joined = join(&actions, player);
        actions.push(joined);
    }
    let started = start(&actions, players[0]);
    actions.push(started);

    for _ in 0..2000 {
        let screens: Vec<_> = players
            .iter()
            .map(|player| (*player, game.show(&actions, *player)))
            .collect();

        if screens.iter().all(|(_, screen)| screen.actions.is_empty()) {
            let winners = screens
                .iter()
                .filter(|(_, screen)| screen.description == "You win!")
                .count();
            let losers = screens
                .iter()
                .filter(|(_, screen)| screen.description == "You lose!")
                .count();
            let draws = screens
                .iter()
                .filter(|(_, screen)| screen.description == "Draw! No one can play.")
                .count();

            let valid = (winners == 1 && losers == players.len() - 1) || draws == players.len();
            assert!(
                valid,
                "unexpected ending: {:?}",
                screens
                    .iter()
                    .map(|(_, screen)| screen.description.clone())
                    .collect::<Vec<_>>()
            );
            return;
        }

        if let Some((player, screen)) = screens
            .iter()
            .find(|(_, screen)| !screen.actions.is_empty())
        {
            let option = &screen.actions[0];
            actions.push(GameAction {
                actor: *player,
                action: option.effect.clone(),
            });
        }
    }

    panic!("game did not terminate within 2000 actions");
}
