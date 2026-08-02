use uuid::Uuid;

use super::{lib_test_support::Counter, BlockClient, BlockParent};

#[test]
fn set_parent_optimistically_notes_backref() {
    let client = BlockClient::new(Uuid::new_v4());
    let child = client.create_block(Counter { count: 0 });
    let parent = Uuid::new_v4();

    child.set_parent(BlockParent::Uuid(parent));
    child.set_parent(BlockParent::Uuid(parent));

    assert_eq!(child.relationships().backrefs, vec![parent]);
}
