use super::{seated, HAND_SIZE};

#[test]
fn playing_a_card_moves_it_from_the_hand_to_the_face_up_card() {
    let (_, mut table) = seated(3);
    let card = table.hand()[0];
    let was_face_up = table.face_up();

    table.pass();
    table.play(card);

    assert_eq!(table.face_up(), card);
    assert_eq!(table.discard_pile, vec![was_face_up, card]);
    assert_eq!(table.hands[0].len(), HAND_SIZE - 1);
    assert!(!table.everyone_has_passed());
}
