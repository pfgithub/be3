use super::seated;

#[test]
fn the_turn_passes_to_the_left_and_comes_back_round() {
    let (players, mut table) = seated(3);

    assert_eq!(table.whose_turn(), players[0]);
    table.pass();
    table.turn_passes_to_the_left();
    assert_eq!(table.whose_turn(), players[1]);
    assert!(!table.everyone_has_passed());
    table.pass();
    table.turn_passes_to_the_left();
    table.pass();
    table.turn_passes_to_the_left();

    assert_eq!(table.whose_turn(), players[0]);
    assert!(table.everyone_has_passed());
}
