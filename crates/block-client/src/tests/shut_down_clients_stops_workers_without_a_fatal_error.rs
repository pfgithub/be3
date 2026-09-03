use super::*;

#[test]
fn shut_down_clients_stops_workers_without_a_fatal_error() {
    let shutdown = Shutdown::new();
    assert!(!shutdown.requested());

    shut_down_clients();

    assert!(shutdown.requested());
    stop_or_fatal(&shutdown, "block server connection failed");
}
