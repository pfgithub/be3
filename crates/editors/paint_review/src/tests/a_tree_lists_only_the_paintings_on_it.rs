use crate::download::paths_in;

#[test]
fn a_tree_lists_only_the_paintings_on_it() {
    let tree = br#"{
        "truncated": false,
        "tree": [
            {"type": "blob", "path": "b.second.paint"},
            {"type": "blob", "path": "a.first.paint"},
            {"type": "blob", "path": "README.md"},
            {"type": "tree", "path": "nested"}
        ]
    }"#;
    assert_eq!(paths_in(tree).unwrap(), ["a.first.paint", "b.second.paint"]);
}
