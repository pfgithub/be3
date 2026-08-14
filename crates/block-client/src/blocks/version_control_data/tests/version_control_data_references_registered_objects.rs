use block::Block;
use uuid::Uuid;

use super::{apply, author, VersionControlData, VersionControlDataOperation};

#[test]
fn version_control_data_references_registered_objects() {
    let mut data = VersionControlData::new(author(), 1_000);
    assert!(data.references().is_empty());

    let hash = super::empty_tree_hash();
    let winner = Uuid::from_u128(0x1);
    let loser = Uuid::from_u128(0x2);

    apply(
        &mut data,
        VersionControlDataOperation::RegisterObject {
            hash: hash.clone(),
            block: winner,
        },
    );
    apply(
        &mut data,
        VersionControlDataOperation::RegisterObject { hash, block: loser },
    );

    assert_eq!(data.references(), vec![winner]);
}
