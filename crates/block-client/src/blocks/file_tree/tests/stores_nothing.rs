use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let tree = FileTree::new();
    assert!(tree.references().is_empty());
    assert_eq!(tree.implicit_name(), None);
    assert_eq!(serde_json::to_string(&tree).unwrap(), "{}");
}
