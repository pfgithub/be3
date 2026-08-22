use uuid::Uuid;

use super::Table;

mod dealing_gives_every_player_a_hand_and_turns_one_card_face_up;
mod drawing_shuffles_the_discard_pile_back_under_the_face_up_card;
mod drawing_with_nothing_left_to_draw_answers_nothing;
mod playing_a_card_moves_it_from_the_hand_to_the_face_up_card;
mod the_turn_passes_to_the_left_and_comes_back_round;

const HAND_SIZE: usize = 8;

fn seated(count: usize) -> (Vec<Uuid>, Table) {
    let players: Vec<Uuid> = (0..count).map(|_| Uuid::new_v4()).collect();
    let table = Table::deal(&players, HAND_SIZE, 1);
    (players, table)
}
