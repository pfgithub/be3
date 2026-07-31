use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{BlockClient, DynamicArtifactDescriptor};

#[derive(Clone, Deserialize, Serialize)]
struct ArtifactTestBlock(u32);

#[derive(Clone, Deserialize, Serialize)]
enum ArtifactTestOperation {}

impl Block for ArtifactTestBlock {
    type Operation = ArtifactTestOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0xda01);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }

    fn implicit_name(&self) -> String {
        "Artifact test".into()
    }
}

#[test]
fn dynamic_artifact_descriptor_survives_creation() {
    let client = BlockClient::new(Uuid::new_v4());
    let descriptor = DynamicArtifactDescriptor {
        source_type: Uuid::new_v4(),
        data: vec![1, 2, 3, 4],
    };
    let block = client.create_dynamic_artifact(ArtifactTestBlock(1), descriptor.clone());

    assert_eq!(block.dynamic_artifact(), Some(descriptor.clone()));
    assert_eq!(client.dynamic_artifact(block.id()), Some(descriptor));
}
