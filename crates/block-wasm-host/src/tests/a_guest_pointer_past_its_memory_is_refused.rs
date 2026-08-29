use super::*;

#[test]
fn a_guest_pointer_past_its_memory_is_refused() {
    let source = guest("(drop (call $create_buffer (i32.const 2000000000) (global.get $length)))");
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    let error = plugin.step().expect_err("the pointer should be refused");
    assert!(error.contains("ran past its memory"), "{error}");
}
