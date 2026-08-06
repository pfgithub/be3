use super::find_matches;

#[test]
fn find_matches_returns_empty_for_empty_query() {
    assert!(find_matches(b"anything at all", "", true).is_empty());
    assert!(find_matches(b"anything at all", "", false).is_empty());
}
