use super::{apply, author, VersionControlData, VersionControlDataOperation, MAIN_BRANCH};

#[test]
fn version_control_data_append_commit_adds_to_history() {
    let mut data = VersionControlData::new(author(), 1_000);
    let parent = data.branch_head(MAIN_BRANCH).cloned();

    apply(
        &mut data,
        VersionControlDataOperation::AppendCommit {
            parent: parent.clone(),
            tree_hash: super::empty_tree_hash(),
            author: author(),
            time: 2_000,
            message: "second commit".to_owned(),
        },
    );

    assert_eq!(data.commits().len(), 2);
    let new_id = data
        .commits()
        .keys()
        .find(|id| *id != parent.as_ref().unwrap())
        .expect("new commit present");
    let commit = data.commit(new_id).unwrap();
    assert_eq!(commit.parent, parent);
    assert_eq!(commit.message, "second commit");

    let before = data.commits().len();
    apply(
        &mut data,
        VersionControlDataOperation::AppendCommit {
            parent,
            tree_hash: super::empty_tree_hash(),
            author: author(),
            time: 2_000,
            message: "second commit".to_owned(),
        },
    );
    assert_eq!(data.commits().len(), before);
}
