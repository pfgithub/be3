use super::*;

#[test]
fn a_guest_reads_a_frame_the_host_queued() {
    let source = guest(
        "(drop (call $host_receive (i32.const 2048) (i32.const 64)))
         (call $host_send (i32.const 2048) (i32.const 4))",
    );
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    plugin.send(vec![7, 8, 9, 10]);
    plugin.step().unwrap();
    assert_eq!(plugin.take_outbound(), vec![vec![7, 8, 9, 10]]);
}
