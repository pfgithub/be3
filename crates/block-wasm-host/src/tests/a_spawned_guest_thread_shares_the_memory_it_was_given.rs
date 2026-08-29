use super::*;

#[test]
fn a_spawned_guest_thread_shares_the_memory_it_was_given() {
    let source = threaded_guest("(i32.atomic.store8 (i32.const 8) (i32.const 42))");
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    plugin.start().unwrap();
    assert_eq!(settled(&mut plugin, 42), Ok(true));
}
