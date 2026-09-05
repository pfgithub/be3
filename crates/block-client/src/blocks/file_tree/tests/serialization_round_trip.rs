use super::*;

#[test]
fn serialization_round_trip() {
    let tree = FileTree::new();
    let encoded = serde_json::to_vec(&tree).unwrap();
    assert_eq!(serde_json::from_slice::<FileTree>(&encoded).unwrap(), tree);
}
