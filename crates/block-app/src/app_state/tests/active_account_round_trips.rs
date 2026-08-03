use super::*;

#[test]
fn active_account_round_trips() {
    let store = AppStateStore::open(":memory:").unwrap();
    let saved = account(ServerLocation::Local, "local@example.com");
    store.save_account(&saved).unwrap();
    store.set_active_account(&saved).unwrap();
    assert_eq!(
        store.active_account().unwrap(),
        Some(("local".into(), saved.id))
    );
}
