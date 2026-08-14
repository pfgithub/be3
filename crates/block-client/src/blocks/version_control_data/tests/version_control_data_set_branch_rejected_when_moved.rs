use super::{apply, author, VersionControlData, VersionControlDataOperation, MAIN_BRANCH};

#[test]
fn version_control_data_set_branch_rejected_when_moved() {
    let mut data = VersionControlData::new(author(), 1_000);
    let initial = data.branch_head(MAIN_BRANCH).cloned().unwrap();

    apply(
        &mut data,
        VersionControlDataOperation::AppendCommit {
            parent: Some(initial.clone()),
            tree_hash: super::empty_tree_hash(),
            author: author(),
            time: 2_000,
            message: "someone else's commit".to_owned(),
        },
    );
    let someone_elses_head = data
        .commits()
        .keys()
        .find(|id| **id != initial)
        .cloned()
        .expect("new commit present");
    apply(
        &mut data,
        VersionControlDataOperation::SetBranch {
            name: MAIN_BRANCH.to_owned(),
            expected: Some(initial.clone()),
            commit: someone_elses_head.clone(),
        },
    );
    assert_eq!(data.branch_head(MAIN_BRANCH), Some(&someone_elses_head));

    // A second commit, built against the now-stale `initial` head, loses the
    // race: applying its `SetBranch` must not move main off the commit that
    // already won.
    apply(
        &mut data,
        VersionControlDataOperation::AppendCommit {
            parent: Some(initial.clone()),
            tree_hash: super::empty_tree_hash(),
            author: author(),
            time: 2_500,
            message: "my commit".to_owned(),
        },
    );
    let my_head = data
        .commits()
        .keys()
        .find(|id| **id != initial && **id != someone_elses_head)
        .cloned()
        .expect("losing commit still appended");
    apply(
        &mut data,
        VersionControlDataOperation::SetBranch {
            name: MAIN_BRANCH.to_owned(),
            expected: Some(initial),
            commit: my_head,
        },
    );

    assert_eq!(data.branch_head(MAIN_BRANCH), Some(&someone_elses_head));
}
