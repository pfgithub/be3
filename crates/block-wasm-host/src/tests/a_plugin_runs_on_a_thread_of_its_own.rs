use super::*;

#[test]
fn a_plugin_runs_on_a_thread_of_its_own() {
    let source = guest(
        "(drop (call $host_receive (i32.const 2048) (i32.const 64)))
         (call $host_send (i32.const 2048) (i32.const 4))",
    );
    let host = host();
    let worker = std::thread::spawn(move || {
        let mut plugin = host.load_bytes(source.as_bytes()).unwrap();
        plugin.start().unwrap();
        plugin.send(vec![1, 2, 3, 4]);
        plugin.step().unwrap();
        plugin.take_outbound()
    });
    assert_eq!(worker.join().unwrap(), vec![vec![1, 2, 3, 4]]);
}
