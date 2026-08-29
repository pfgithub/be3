use super::*;

#[test]
fn a_dropped_handle_is_reported_not_reused() {
    let mut gpu = gpu();
    let handle = gpu.create_buffer(&buffer_descriptor());
    gpu.drop_resource(abi::ResourceKind::Buffer.code(), handle);
    gpu.write_buffer(handle, 0, &[0u8; 16]);
    let error = gpu.take_error().expect("the dropped handle should report");
    assert!(error.contains("buffer"), "{error}");
    let next = gpu.create_buffer(&buffer_descriptor());
    assert_ne!(next, handle);
}
