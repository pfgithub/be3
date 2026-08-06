use super::find_matches;

#[test]
fn find_matches_respects_case_sensitive_flag() {
    let matches = find_matches(b"Wow wow WOW", "wow", true);

    assert_eq!(matches, vec![4..7]);
}
