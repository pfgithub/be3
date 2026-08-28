use crate::download::paths_in;

#[test]
fn a_tree_lists_only_the_paintings_on_it() {
    let tree = br#"{
        "truncated": false,
        "tree": [
            {"type": "blob", "path": "crates/b/snapshots/second.paint"},
            {"type": "blob", "path": "crates/a/snapshots/first.paint"},
            {"type": "blob", "path": "crates/a/src/app.rs"},
            {"type": "tree", "path": "crates/a/snapshots"}
        ]
    }"#;
    assert_eq!(
        paths_in(tree).unwrap(),
        [
            "crates/a/snapshots/first.paint",
            "crates/b/snapshots/second.paint"
        ]
    );
}
