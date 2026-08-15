use super::*;

#[test]
fn short_commit_id_truncates_to_short_id_len() {
    let commit = Commit {
        parent: None,
        tree_hash: block_client::blocks::version_control_object::ObjectHash::of(b"tree"),
        author: uuid::Uuid::from_u128(1),
        time: 0,
        message: "Initial commit".to_owned(),
    };
    let id = commit.id();
    let short = short_commit_id(&id);
    assert_eq!(short.len(), SHORT_ID_LEN);
    assert_eq!(short, id.as_str()[..SHORT_ID_LEN]);
}
