use uuid::Uuid;

use super::{PixelRayTracer, PixelRayTracerOperation, Point, RayEntity};
use crate::BlockClient;

#[test]
fn pixel_ray_tracer_history_undoes_and_redoes_entity_changes() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(PixelRayTracer::new());
    let original = RayEntity::Light {
        id: 1,
        position: Point::new(10.0, 20.0),
        color_index: 7,
        intensity: 2.0,
    };
    let updated = RayEntity::Light {
        id: 1,
        position: Point::new(30.0, 40.0),
        color_index: 10,
        intensity: 3.0,
    };

    block.operate(PixelRayTracerOperation::AddEntity {
        entity: original.clone(),
    });
    block.operate(PixelRayTracerOperation::UpdateEntity {
        entity: updated.clone(),
    });
    block.undo();
    assert_eq!(block.read().unwrap().entities(), &[original.clone()]);
    block.redo();
    assert_eq!(block.read().unwrap().entities(), &[updated.clone()]);
    block.operate(PixelRayTracerOperation::DeleteEntity { id: 1 });
    block.undo();
    assert_eq!(block.read().unwrap().entities(), &[updated]);
    block.redo();
    assert!(block.read().unwrap().entities().is_empty());
}
