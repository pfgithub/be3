use uuid::Uuid;

use super::{ObjectHash, TreeEntry, TreeEntryKind, VersionControlObject};

#[test]
fn version_control_object_trees_with_different_entries_hash_differently() {
    let base = TreeEntry {
        eternal_id: Uuid::from_u128(1),
        kind: TreeEntryKind::Blob,
        content_hash: ObjectHash::of(b"child content"),
        name: "file.txt".to_owned(),
    };
    let renamed = TreeEntry {
        name: "renamed.txt".to_owned(),
        ..base.clone()
    };
    let different_target = TreeEntry {
        content_hash: ObjectHash::of(b"different child content"),
        ..base.clone()
    };

    let original = VersionControlObject::tree(vec![base]);
    let with_rename = VersionControlObject::tree(vec![renamed]);
    let with_different_target = VersionControlObject::tree(vec![different_target]);

    assert_ne!(original.content_hash(), with_rename.content_hash());
    assert_ne!(
        original.content_hash(),
        with_different_target.content_hash()
    );
}
