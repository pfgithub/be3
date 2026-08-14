use block::BlockParent;

use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::VersionControlWorktree;
use crate::blocks::workspace_index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation};

use super::{materialize_worktree, CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_second_worktree_materializes_existing_repo_content() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let nested = fixture.client.create_block(WorkspaceIndex::default());
    nested.loaded().await;
    nested.set_parent(BlockParent::Uuid(member_id));
    fixture
        .client
        .get_block::<WorkspaceIndex>(member_id)
        .operate(WorkspaceIndexOperation::Add(BlockEntry { id: nested.id() }));
    fixture.client.synchronized().await;
    fixture.commit("first").await;

    let data_snapshot = fixture
        .client
        .get_block::<VersionControlData>(fixture.data_id)
        .read()
        .unwrap()
        .clone();
    let second_worktree = fixture
        .client
        .create_block(VersionControlWorktree::new(fixture.data_id, &data_snapshot));
    second_worktree.loaded().await;
    second_worktree.set_parent(BlockParent::Root);
    fixture.client.synchronized().await;

    let outcome = materialize_worktree(&fixture.client, second_worktree.id())
        .await
        .expect("materialize_worktree should succeed");

    let CheckoutOutcome::Applied { created, .. } = outcome else {
        panic!("expected checkout to be applied");
    };
    assert_eq!(created.len(), 1);
    let materialized_id = created[0];
    assert_ne!(materialized_id, member_id);

    assert_eq!(second_worktree.read().unwrap().members().count(), 1);

    let original = fixture
        .client
        .get_block::<WorkspaceIndex>(member_id)
        .read()
        .unwrap()
        .entries()
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    let materialized = fixture
        .client
        .get_block::<WorkspaceIndex>(materialized_id)
        .read()
        .unwrap()
        .entries()
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    assert_eq!(original, materialized);

    fixture.tear_down().await;
}
