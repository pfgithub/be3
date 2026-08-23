use std::convert::Infallible;

use uuid::Uuid;

use game_api::{GameHelper, GameScreen};

const COLUMNS: usize = 7;
const ROWS: usize = 6;
const CELL_COUNT: usize = COLUMNS * ROWS;

const DIRECTIONS: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Symbol {
    Red,
    Yellow,
}

impl Symbol {
    fn label(self) -> &'static str {
        match self {
            Symbol::Red => "Red",
            Symbol::Yellow => "Yellow",
        }
    }
}

fn cell(column: usize, row: usize) -> usize {
    row * COLUMNS + column
}

fn column_label(column: usize) -> String {
    format!("Column {}", column + 1)
}

fn in_bounds(column: isize, row: isize) -> bool {
    (0..COLUMNS as isize).contains(&column) && (0..ROWS as isize).contains(&row)
}

fn winning_symbol(board: &[Option<Symbol>; CELL_COUNT]) -> Option<Symbol> {
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let Some(symbol) = board[cell(column, row)] else {
                continue;
            };
            for (delta_column, delta_row) in DIRECTIONS {
                let in_a_row = (0..4).all(|step| {
                    let column = column as isize + delta_column * step;
                    let row = row as isize + delta_row * step;
                    in_bounds(column, row)
                        && board[cell(column as usize, row as usize)] == Some(symbol)
                });
                if in_a_row {
                    return Some(symbol);
                }
            }
        }
    }
    None
}

fn connect_four(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut board: [Option<Symbol>; CELL_COUNT] = [None; CELL_COUNT];
    let mut column_heights: [usize; COLUMNS] = [0; COLUMNS];
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
        let symbol = if turn == 0 {
            Symbol::Red
        } else {
            Symbol::Yellow
        };
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
                for column in 0..COLUMNS {
                    if column_heights[column] < ROWS && action(&column_label(column)) {
                        if players[turn].is_none() {
                            players[turn] = Some(player);
                        }
                        let row = column_heights[column];
                        board[cell(column, row)] = Some(symbol);
                        column_heights[column] += 1;
                        return;
                    }
                }
            },
        )?;

        move_count += 1;
    }
}

game_api::game!("Connect Four", connect_four);

#[cfg(test)]
mod tests;
