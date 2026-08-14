use super::{apply, author, VersionControlData, VersionControlDataOperation, MAIN_BRANCH};

#[test]
fn version_control_data_set_branch_creates_new_branch() {
    let mut data = VersionControlData::new(author(), 1_000);
    let initial = data.branch_head(MAIN_BRANCH).cloned().unwrap();

    apply(
        &mut data,
        VersionControlDataOperation::SetBranch {
            name: "feature".to_owned(),
            expected: None,
            commit: initial.clone(),
        },
    );

    assert_eq!(data.branch_head("feature"), Some(&initial));
    assert_eq!(data.branches().len(), 2);

    apply(
        &mut data,
        VersionControlDataOperation::SetBranch {
            name: "feature".to_owned(),
            expected: None,
            commit: initial.clone(),
        },
    );

    assert_eq!(data.branch_head("feature"), Some(&initial));
    assert_eq!(data.branches().len(), 2);
}
