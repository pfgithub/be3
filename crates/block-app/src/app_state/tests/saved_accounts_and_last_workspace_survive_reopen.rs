use super::*;

#[test]
fn saved_accounts_and_last_workspace_survive_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let mut local = account(ServerLocation::Local, "local@example.com");
    let remote = account(
        ServerLocation::Remote("wss://blocks.example.com".into()),
        "remote@example.com",
    );
    let workspace_id = Uuid::new_v4();
    {
        let store = AppStateStore::open(&path).unwrap();
        store.save_account(&local).unwrap();
        store.save_account(&remote).unwrap();
        store
            .set_last_workspace(&local, Some(workspace_id))
            .unwrap();
    }
    local.last_workspace_id = Some(workspace_id);
    let store = AppStateStore::open(&path).unwrap();
    let accounts = store.accounts().unwrap();
    assert!(accounts.contains(&local));
    assert!(accounts.contains(&remote));
    drop(store);
    std::fs::remove_file(path).unwrap();
}
