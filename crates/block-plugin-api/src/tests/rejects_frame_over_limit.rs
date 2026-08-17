use super::*;

#[test]
fn rejects_frame_over_limit() {
    let mut frame = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&[0; 8]);
    assert_eq!(
        decode_frame(&frame),
        Err(DecodeError::FrameTooLarge {
            length: MAX_FRAME_BYTES + 1,
            maximum: MAX_FRAME_BYTES
        })
    );
}
