use super::super::core::scan_matches;

#[test]
fn find_matches_matches_are_non_overlapping() {
    let matches = scan_matches(b"aaaa", "aa", true);

    assert_eq!(matches, vec![0..2, 2..4]);
}
