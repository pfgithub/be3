use super::find_matches;

#[test]
fn find_matches_is_case_insensitive_by_default() {
    let matches = find_matches(b"Wow wow WOW", "wow", false);

    assert_eq!(matches, vec![0..3, 4..7, 8..11]);
}
