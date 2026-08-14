use uuid::Uuid;

use super::VersionControlObject;

#[test]
fn version_control_object_differing_blobs_hash_differently() {
    let source_block_type = Uuid::from_u128(0x1234);

    let first = VersionControlObject::blob(source_block_type, b"content a".to_vec());
    let second = VersionControlObject::blob(source_block_type, b"content b".to_vec());
    let different_type = VersionControlObject::blob(Uuid::from_u128(0x5678), b"content a".to_vec());

    assert_ne!(first.content_hash(), second.content_hash());
    assert_ne!(first.content_hash(), different_type.content_hash());
}
