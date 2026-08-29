use super::*;

#[test]
fn a_guest_creates_a_buffer_and_reports_its_handle() {
    let (device, queue) = devices();
    let source = guest(
        "(i32.store (i32.const 1024) (call $create_buffer (i32.const 0) (global.get $length)))
         (call $host_send (i32.const 1024) (i32.const 4))",
    );
    let mut plugin = Plugin::from_bytes(source.as_bytes(), device, queue).unwrap();
    plugin.start().unwrap();
    plugin.step().unwrap();
    let outbound = plugin.take_outbound();
    assert_eq!(outbound.len(), 1);
    let handle = u32::from_le_bytes(outbound[0][..4].try_into().unwrap());
    assert_ne!(handle, abi::NULL_HANDLE);
}
