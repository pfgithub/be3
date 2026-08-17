use super::*;

#[test]
fn rejects_collection_over_limit() {
    let message = Message::Input(InputBatch {
        viewport_request_id: 1,
        events: vec![InputEvent::Focus(true); MAX_COLLECTION_ITEMS + 1],
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("collection"))
    );
}
