use super::*;

#[test]
fn removing_an_account_clears_active_selection() {
    let store = AppStateStore::open(":memory:").unwrap();
    let saved = account(ServerLocation::Local, "local@example.com");
    store.save_account(&saved).unwrap();
    store.set_active_account(&saved).unwrap();
    store.remove_account(&saved).unwrap();
    assert!(store.accounts().unwrap().is_empty());
    assert_eq!(store.active_account().unwrap(), None);
}
