use super::find_matches;

#[test]
fn find_matches_matches_are_non_overlapping() {
    let matches = find_matches(b"aaaa", "aa", true);

    assert_eq!(matches, vec![0..2, 2..4]);
}
