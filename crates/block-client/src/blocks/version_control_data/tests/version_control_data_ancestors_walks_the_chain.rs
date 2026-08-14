use super::{apply, author, VersionControlData, VersionControlDataOperation, MAIN_BRANCH};

#[test]
fn version_control_data_ancestors_walks_the_chain() {
    let mut data = VersionControlData::new(author(), 1_000);
    let root = data.branch_head(MAIN_BRANCH).cloned().unwrap();

    let mut parent = root.clone();
    let mut chain = vec![root.clone()];
    for time in [2_000, 3_000, 4_000] {
        apply(
            &mut data,
            VersionControlDataOperation::AppendCommit {
                parent: Some(parent.clone()),
                tree_hash: super::empty_tree_hash(),
                author: author(),
                time,
                message: format!("commit at {time}"),
            },
        );
        let next = data
            .commits()
            .keys()
            .find(|id| !chain.contains(id))
            .cloned()
            .expect("new commit present");
        chain.push(next.clone());
        parent = next;
    }

    let ancestors = data.ancestors(&parent);
    let expected: Vec<_> = chain.into_iter().rev().collect();
    assert_eq!(ancestors, expected);
}
