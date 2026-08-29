use super::*;

#[test]
fn a_malformed_descriptor_is_reported() {
    let mut gpu = gpu();
    assert_eq!(gpu.create_buffer(&[0u8; 3]), abi::NULL_HANDLE);
    assert!(gpu.take_error().is_some());
}
