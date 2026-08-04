use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{BlockClient, DynamicArtifactDescriptor};

#[derive(Clone, Deserialize, Serialize)]
struct UnlinkTestBlock(u32);

#[derive(Clone, Deserialize, Serialize)]
enum UnlinkTestOperation {}

impl Block for UnlinkTestBlock {
    type Operation = UnlinkTestOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0xda04);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }

    fn implicit_name(&self) -> String {
        format!("Unlink test {}", self.0)
    }
}

#[test]
fn clear_dynamic_artifact_unlinks_the_block() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_dynamic_artifact(
        UnlinkTestBlock(1),
        DynamicArtifactDescriptor {
            source_type: Uuid::new_v4(),
            data: vec![1],
        },
    );
    let revision = block.revision();

    client.clear_dynamic_artifact(block.id());

    assert_eq!(block.dynamic_artifact(), None);
    assert_eq!(client.dynamic_artifact(block.id()), None);
    assert!(!client.is_dynamic_artifact(block.id()));
    assert!(block.revision() > revision);
    // Unlinking leaves the generated value behind.
    assert_eq!(block.read().unwrap().0, 1);
}
