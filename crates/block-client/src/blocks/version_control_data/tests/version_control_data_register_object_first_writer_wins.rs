use uuid::Uuid;

use super::{apply, author, VersionControlData, VersionControlDataOperation};

#[test]
fn version_control_data_register_object_first_writer_wins() {
    let mut data = VersionControlData::new(author(), 1_000);
    let hash = super::empty_tree_hash();
    assert!(!data.contains_object(&hash));
    assert_eq!(data.object_id(&hash), None);

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
        VersionControlDataOperation::RegisterObject {
            hash: hash.clone(),
            block: loser,
        },
    );

    assert!(data.contains_object(&hash));
    assert_eq!(data.object_id(&hash), Some(winner));
}
