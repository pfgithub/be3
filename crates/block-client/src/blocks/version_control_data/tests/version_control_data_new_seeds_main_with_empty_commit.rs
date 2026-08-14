use super::{author, empty_tree_hash, VersionControlData, MAIN_BRANCH};

#[test]
fn version_control_data_new_seeds_main_with_empty_commit() {
    let data = VersionControlData::new(author(), 1_000);

    let head = data.branch_head(MAIN_BRANCH).expect("main branch exists");
    let commit = data.commit(head).expect("initial commit exists");

    assert!(commit.parent.is_none());
    assert_eq!(commit.tree_hash, empty_tree_hash());
    assert_eq!(commit.author, author());
    assert_eq!(commit.time, 1_000);
    assert_eq!(data.commits().len(), 1);
    assert_eq!(data.branches().len(), 1);
}
