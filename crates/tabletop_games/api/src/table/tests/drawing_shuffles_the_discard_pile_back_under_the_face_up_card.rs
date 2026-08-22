use std::mem::take;

use super::{seated, HAND_SIZE};

#[test]
fn drawing_shuffles_the_discard_pile_back_under_the_face_up_card() {
    let (_, mut table) = seated(2);
    let face_up = table.face_up();
    table.discard_pile.pop();
    table.discard_pile.append(&mut take(&mut table.draw_pile));
    table.discard_pile.push(face_up);
    let left_to_draw = table.discard_pile.len() - 1;

    let drawn = table.draw().expect("the discard pile still holds cards");

    assert_ne!(drawn, face_up);
    assert_eq!(table.hands[0].len(), HAND_SIZE + 1);
    assert_eq!(table.draw_pile.len(), left_to_draw - 1);
    assert_eq!(table.discard_pile, vec![face_up]);
}
