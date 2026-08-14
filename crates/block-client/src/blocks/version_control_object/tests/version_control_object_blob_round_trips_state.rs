use uuid::Uuid;

use super::{ObjectPayload, VersionControlObject};

#[test]
fn version_control_object_blob_round_trips_state() {
    let source_block_type = Uuid::from_u128(0x1234);
    let state = b"hello world".to_vec();
    let object = VersionControlObject::blob(source_block_type, state.clone());

    let encoded = serde_json::to_vec(&object).unwrap();
    let decoded: VersionControlObject = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, object);
    let ObjectPayload::Blob {
        source_block_type: decoded_type,
        state: decoded_state,
    } = decoded.payload()
    else {
        panic!("expected a blob");
    };
    assert_eq!(*decoded_type, source_block_type);
    assert_eq!(*decoded_state, state);
}
