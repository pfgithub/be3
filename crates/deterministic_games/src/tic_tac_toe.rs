use uuid::Uuid;

use crate::{Game, GameAction, GameActionOption, GameScreen};

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

struct Replay {
    board: [Option<Symbol>; CELL_COUNT],
    players: [Option<Uuid>; 2],
    move_count: usize,
    winner: Option<Symbol>,
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

/// Replays the log from an empty board. Actions that are malformed, out of
/// turn, played by the wrong actor, or on an occupied cell are silently
/// skipped rather than rejected: the log has no separate validation step, so
/// every viewer must reach the same board by ignoring the same nonsense.
fn replay(actions: &[GameAction]) -> Replay {
    let mut board = [None; CELL_COUNT];
    let mut players: [Option<Uuid>; 2] = [None, None];
    let mut move_count = 0;
    let mut winner = None;
    for action in actions {
        if winner.is_some() || move_count >= CELL_COUNT {
            break;
        }
        let &[cell] = action.action.as_slice() else {
            continue;
        };
        let cell = cell as usize;
        if cell >= CELL_COUNT || board[cell].is_some() {
            continue;
        }
        let turn = move_count % 2;
        match players[turn] {
            Some(expected) if expected != action.actor => continue,
            None if players[1 - turn] == Some(action.actor) => continue,
            None => players[turn] = Some(action.actor),
            _ => {}
        }
        board[cell] = Some(if turn == 0 { Symbol::X } else { Symbol::O });
        move_count += 1;
        winner = winning_symbol(&board);
    }
    Replay {
        board,
        players,
        move_count,
        winner,
    }
}

impl Game for TicTacToe {
    fn show(&self, actions: &[GameAction], player: Uuid) -> GameScreen {
        let replay = replay(actions);
        if let Some(winner) = replay.winner {
            return GameScreen {
                description: format!("{} wins!", winner.label()),
                actions: Vec::new(),
            };
        }
        if replay.move_count >= CELL_COUNT {
            return GameScreen {
                description: "Draw!".to_owned(),
                actions: Vec::new(),
            };
        }
        let turn = replay.move_count % 2;
        let symbol = if turn == 0 { Symbol::X } else { Symbol::O };
        let can_move = match replay.players[turn] {
            Some(expected) => expected == player,
            None => replay.players[1 - turn] != Some(player),
        };
        if !can_move {
            return GameScreen {
                description: format!("Waiting for {}...", symbol.label()),
                actions: Vec::new(),
            };
        }
        let actions = replay
            .board
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.is_none())
            .map(|(index, _)| GameActionOption {
                label: format!("Row {}, column {}", index / 3 + 1, index % 3 + 1),
                effect: vec![index as u8],
            })
            .collect();
        GameScreen {
            description: format!("Your turn ({})", symbol.label()),
            actions,
        }
    }
}

#[cfg(test)]
mod tests;
