use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{BlockClient, DynamicArtifactDescriptor};

#[derive(Clone, Deserialize, Serialize)]
struct ReplaceTestBlock(u32);

#[derive(Clone, Deserialize, Serialize)]
enum ReplaceTestOperation {}

impl Block for ReplaceTestBlock {
    type Operation = ReplaceTestOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0xda02);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }

    fn implicit_name(&self) -> String {
        format!("Replace test {}", self.0)
    }
}

#[test]
fn replace_preserves_dynamic_artifact_descriptor() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let descriptor = DynamicArtifactDescriptor {
        source_type: Uuid::new_v4(),
        data: vec![5, 6, 7, 8],
    };
    let block = client.create_dynamic_artifact(ReplaceTestBlock(1), descriptor.clone());
    let revision = block.revision();

    block.replace(ReplaceTestBlock(2));

    assert_eq!(block.read().unwrap().0, 2);
    assert!(block.revision() > revision);
    assert_eq!(block.dynamic_artifact(), Some(descriptor));
}
