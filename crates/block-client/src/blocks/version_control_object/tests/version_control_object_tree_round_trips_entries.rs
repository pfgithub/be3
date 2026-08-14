use uuid::Uuid;

use super::{ObjectHash, ObjectPayload, TreeEntry, TreeEntryKind, VersionControlObject};

#[test]
fn version_control_object_tree_round_trips_entries() {
    let entry = TreeEntry {
        eternal_id: Uuid::from_u128(1),
        kind: TreeEntryKind::Blob,
        content_hash: ObjectHash::of(b"child content"),
        name: "file.txt".to_owned(),
    };
    let object = VersionControlObject::tree(vec![entry.clone()]);

    let encoded = serde_json::to_vec(&object).unwrap();
    let decoded: VersionControlObject = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, object);
    let ObjectPayload::Tree { entries } = decoded.payload() else {
        panic!("expected a tree");
    };
    assert_eq!(entries, &vec![entry]);
}
