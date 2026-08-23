use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{BlockClient, DynamicArtifactDescriptor};

#[derive(Clone, Deserialize, Serialize)]
struct SettingsTestBlock(u32);

#[derive(Clone, Deserialize, Serialize)]
enum SettingsTestOperation {}

impl Block for SettingsTestBlock {
    type Operation = SettingsTestOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0xda03);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }

    fn implicit_name(&self) -> Option<String> {
        Some(format!("Settings test {}", self.0))
    }
}

#[test]
fn set_dynamic_artifact_updates_the_descriptor() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let source_type = Uuid::new_v4();
    let block = client.create_dynamic_artifact(
        SettingsTestBlock(1),
        DynamicArtifactDescriptor {
            source_type,
            data: vec![1],
        },
    );
    let revision = block.revision();

    let updated = DynamicArtifactDescriptor {
        source_type,
        data: vec![2, 3],
    };
    client.set_dynamic_artifact(block.id(), updated.clone());

    assert_eq!(block.dynamic_artifact(), Some(updated.clone()));
    assert_eq!(client.dynamic_artifact(block.id()), Some(updated));
    assert!(block.revision() > revision);

    assert_eq!(block.read().unwrap().0, 1);
}
