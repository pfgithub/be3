use super::{seated, HAND_SIZE};

#[test]
fn drawing_with_nothing_left_to_draw_answers_nothing() {
    let (_, mut table) = seated(2);
    let face_up = table.face_up();
    table.draw_pile.clear();

    assert!(!table.can_draw());
    assert!(table.draw().is_none());
    assert_eq!(table.hands[0].len(), HAND_SIZE);
    assert_eq!(table.face_up(), face_up);
}
