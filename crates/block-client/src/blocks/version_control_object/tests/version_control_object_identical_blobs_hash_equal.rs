use uuid::Uuid;

use super::VersionControlObject;

#[test]
fn version_control_object_identical_blobs_hash_equal() {
    let source_block_type = Uuid::from_u128(0x1234);
    let state = b"same content".to_vec();

    let first = VersionControlObject::blob(source_block_type, state.clone());
    let second = VersionControlObject::blob(source_block_type, state);

    assert_eq!(first.content_hash(), second.content_hash());
}
