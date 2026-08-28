use crate::download::paths_in;

#[test]
fn a_tree_that_says_nothing_useful_is_an_error() {
    let refused = br#"{"message": "API rate limit exceeded"}"#;
    assert!(paths_in(refused)
        .unwrap_err()
        .contains("API rate limit exceeded"));

    let truncated = br#"{"truncated": true, "tree": []}"#;
    assert!(paths_in(truncated).is_err());

    let nonsense = b"not json";
    assert!(paths_in(nonsense).is_err());
}
