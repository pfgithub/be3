use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ActionLabel, Game, GameAction, GameHelper, GameScreen};

pub struct TicTacToe;

const CELL_COUNT: usize = 9;

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Symbol {
    X,
    O,
}

impl Symbol {
    fn label(self) -> &'static str {
        match self {
            Symbol::X => "X",
            Symbol::O => "O",
        }
    }
}

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
struct PlayAction {
    player: Uuid,
    cell: u8,
}

impl ActionLabel for PlayAction {
    fn label(&self) -> String {
        format!("Row {}, column {}", self.cell / 3 + 1, self.cell % 3 + 1)
    }
}

fn winning_symbol(board: &[Option<Symbol>; CELL_COUNT]) -> Option<Symbol> {
    for line in LINES {
        let [a, b, c] = line.map(|index| board[index]);
        if a.is_some() && a == b && b == c {
            return a;
        }
    }
    None
}

/// The whole game as one straight-line function over the action log:
/// `helper.action` blocks on a player until the log supplies their move, so
/// the code below reads like a normal loop rather than a hand-written replay
/// pass. Player identity is carried inside `PlayAction` itself (set from the
/// candidate the options closure was asked about), which is what lets the
/// game learn who actually made a move without `GameHelper` exposing an
/// actor separately.
fn tic_tac_toe(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut board: [Option<Symbol>; CELL_COUNT] = [None; CELL_COUNT];
    let mut players: [Option<Uuid>; 2] = [None, None];
    let mut move_count = 0;

    loop {
        if let Some(winner) = winning_symbol(&board) {
            helper.action::<PlayAction>(
                move |_| format!("{} wins!", winner.label()),
                |_| Vec::new(),
            )?;
        }
        if move_count >= CELL_COUNT {
            helper.action::<PlayAction>(|_| "Draw!".to_owned(), |_| Vec::new())?;
        }

        let turn = move_count % 2;
        let symbol = if turn == 0 { Symbol::X } else { Symbol::O };
        let expected = players[turn];
        let other = players[1 - turn];
        let can_move = move |player: Uuid| match expected {
            Some(expected) => expected == player,
            None => other != Some(player),
        };
        let snapshot = board;

        let play = helper.action::<PlayAction>(
            move |player| {
                if can_move(player) {
                    format!("Your turn ({})", symbol.label())
                } else {
                    format!("Waiting for {}...", symbol.label())
                }
            },
            move |player| {
                if !can_move(player) {
                    return Vec::new();
                }
                snapshot
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| cell.is_none())
                    .map(|(cell, _)| PlayAction {
                        player,
                        cell: cell as u8,
                    })
                    .collect()
            },
        )?;

        if players[turn].is_none() {
            players[turn] = Some(play.player);
        }
        board[play.cell as usize] = Some(symbol);
        move_count += 1;
    }
}

impl Game for TicTacToe {
    fn show(&self, actions: &[GameAction], player: Uuid) -> GameScreen {
        match tic_tac_toe(GameHelper::new(actions, player)) {
            Ok(never) => match never {},
            Err(screen) => screen,
        }
    }
}

#[cfg(test)]
mod tests;
