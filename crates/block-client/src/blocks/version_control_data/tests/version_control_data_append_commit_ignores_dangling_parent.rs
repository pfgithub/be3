use super::{apply, author, CommitId, VersionControlData, VersionControlDataOperation};

#[test]
fn version_control_data_append_commit_ignores_dangling_parent() {
    let mut data = VersionControlData::new(author(), 1_000);
    let before = data.commits().len();

    apply(
        &mut data,
        VersionControlDataOperation::AppendCommit {
            parent: Some(CommitId::default()),
            tree_hash: super::empty_tree_hash(),
            author: author(),
            time: 2_000,
            message: "orphaned parent".to_owned(),
        },
    );

    assert_eq!(data.commits().len(), before);
}
