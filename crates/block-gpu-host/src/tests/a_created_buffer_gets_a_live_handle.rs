use super::*;

#[test]
fn a_created_buffer_gets_a_live_handle() {
    let mut gpu = gpu();
    let handle = gpu.create_buffer(&buffer_descriptor());
    assert_ne!(handle, abi::NULL_HANDLE);
    assert_eq!(gpu.take_error(), None);
    gpu.write_buffer(handle, 0, &[0u8; 16]);
    assert_eq!(gpu.take_error(), None);
}
