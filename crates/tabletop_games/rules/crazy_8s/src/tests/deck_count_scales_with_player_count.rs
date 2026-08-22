use super::deck_count_for;

#[test]
fn deck_count_scales_with_player_count() {
    assert_eq!(deck_count_for(1), 1);
    assert_eq!(deck_count_for(4), 1);
    assert_eq!(deck_count_for(5), 2);
    assert_eq!(deck_count_for(8), 2);
    assert_eq!(deck_count_for(9), 3);
}
