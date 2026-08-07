use super::*;

#[test]
fn client_id_persists_across_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let first = {
        let store = AppStateStore::open(&path).unwrap();
        store.client_id().unwrap()
    };
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(store.client_id().unwrap(), first);
    drop(store);
    std::fs::remove_file(path).unwrap();
}
