use super::decks_for;

#[test]
fn decks_scale_with_the_number_of_players() {
    assert_eq!(decks_for(2), 1);
    assert_eq!(decks_for(4), 1);
    assert_eq!(decks_for(5), 2);
    assert_eq!(decks_for(8), 2);
    assert_eq!(decks_for(9), 3);
}
