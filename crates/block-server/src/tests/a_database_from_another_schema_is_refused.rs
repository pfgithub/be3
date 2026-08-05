use super::*;

/// Stored blocks are never migrated. A database written by a different version
/// of the schema is left exactly as it is: the server refuses to open it rather
/// than dropping what it cannot read.
#[tokio::test]
async fn a_database_from_another_schema_is_refused() {
    let root = test_root();
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("server.sqlite3");
    {
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE blocks (
                    workspace_id    TEXT NOT NULL,
                    id              TEXT NOT NULL,
                    snapshot        BLOB NOT NULL,
                    PRIMARY KEY (workspace_id, id)
                );
                 INSERT INTO blocks (workspace_id, id, snapshot) VALUES ('w', 'b', x'00');",
            )
            .unwrap();
    }

    let Err(error) = BlockStore::open_with_config(root.clone(), ServerConfig::default()).await
    else {
        panic!("a database from another schema must not open");
    };
    let message = error.to_string();
    assert!(message.contains("blocks"), "unexpected error: {message}");

    // The rows the server could not read are still there.
    let connection = Connection::open(&database).unwrap();
    let blocks: i64 = connection
        .query_row("SELECT count(*) FROM blocks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(blocks, 1);
    drop(connection);
    fs::remove_dir_all(root).await.unwrap();
}
