use uuid::Uuid;

use super::{ObjectHash, TreeEntry, TreeEntryKind, VersionControlObject};

fn entry() -> TreeEntry {
    TreeEntry {
        eternal_id: Uuid::from_u128(1),
        kind: TreeEntryKind::Blob,
        content_hash: ObjectHash::of(b"child content"),
        name: "file.txt".to_owned(),
    }
}

#[test]
fn version_control_object_identical_trees_hash_equal() {
    let first = VersionControlObject::tree(vec![entry()]);
    let second = VersionControlObject::tree(vec![entry()]);

    assert_eq!(first.content_hash(), second.content_hash());
}
