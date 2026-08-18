use std::convert::Infallible;

use uuid::Uuid;

use crate::{Game, GameAction, GameHelper, GameScreen};

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

fn cell_label(cell: usize) -> String {
    format!("Row {}, column {}", cell / 3 + 1, cell % 3 + 1)
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
/// pass. Each legal move is offered by calling `action(label)`; when it
/// returns `true` the move it names is the one the log records next for that
/// actor, so the board and player-assignment updates happen right there,
/// inline, instead of being decoded from a returned value afterward.
fn tic_tac_toe(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut board: [Option<Symbol>; CELL_COUNT] = [None; CELL_COUNT];
    let mut players: [Option<Uuid>; 2] = [None, None];
    let mut move_count = 0;

    loop {
        if let Some(winner) = winning_symbol(&board) {
            helper.action(move |_| format!("{} wins!", winner.label()), |_, _| {})?;
        }
        if move_count >= CELL_COUNT {
            helper.action(|_| "Draw!".to_owned(), |_, _| {})?;
        }

        let turn = move_count % 2;
        let symbol = if turn == 0 { Symbol::X } else { Symbol::O };
        let expected = players[turn];
        let other = players[1 - turn];
        let can_move = move |player: Uuid| match expected {
            Some(expected) => expected == player,
            None => other != Some(player),
        };

        helper.action(
            move |player| {
                if can_move(player) {
                    format!("Your turn ({})", symbol.label())
                } else {
                    format!("Waiting for {}...", symbol.label())
                }
            },
            |player, action| {
                if !can_move(player) {
                    return;
                }
                for cell in 0..CELL_COUNT {
                    if board[cell].is_none() && action(&cell_label(cell)) {
                        if players[turn].is_none() {
                            players[turn] = Some(player);
                        }
                        board[cell] = Some(symbol);
                        return;
                    }
                }
            },
        )?;

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
