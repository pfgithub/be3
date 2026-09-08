use super::*;

#[test]
fn shut_down_clients_stops_workers_without_a_fatal_error() {
    let registry = ShutdownRegistry::new();
    let shutdown = Shutdown::in_registry(&registry);
    assert!(!shutdown.requested());

    registry.shut_down_all();

    assert!(shutdown.requested());
    stop_or_fatal(&shutdown, "block server connection failed");
}
