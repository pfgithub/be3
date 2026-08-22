use super::{seated, HAND_SIZE};

#[test]
fn dealing_gives_every_player_a_hand_and_turns_one_card_face_up() {
    let (players, table) = seated(3);

    assert_eq!(table.hands.len(), players.len());
    for hand in &table.hands {
        assert_eq!(hand.len(), HAND_SIZE);
    }
    assert_eq!(table.draw_pile.len(), 52 - players.len() * HAND_SIZE - 1);
    assert_eq!(table.discard_pile, vec![table.face_up()]);
    assert_eq!(table.whose_turn(), players[0]);
    assert_eq!(table.hand(), table.hands[0]);
    assert!(table.player_who_is_out().is_none());
}
