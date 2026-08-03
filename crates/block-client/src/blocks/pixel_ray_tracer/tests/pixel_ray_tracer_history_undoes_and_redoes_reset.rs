use uuid::Uuid;

use super::{PixelRayTracer, PixelRayTracerOperation, PixelUpdate, Point, RayEntity};
use crate::BlockClient;

#[test]
fn pixel_ray_tracer_history_undoes_and_redoes_reset() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(PixelRayTracer::new());
    block.operate(PixelRayTracerOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 1,
            y: 2,
            color_index: 8,
        }],
    });
    block.operate(PixelRayTracerOperation::AddEntity {
        entity: RayEntity::Water {
            id: 1,
            start: Point::new(1.0, 2.0),
            end: Point::new(3.0, 4.0),
        },
    });

    block.operate(PixelRayTracerOperation::Reset);
    block.undo();
    let restored = block.read().unwrap();
    assert_eq!(restored.pixels()[2 * 128 + 1], 8);
    assert_eq!(restored.entities().len(), 1);
    drop(restored);
    block.redo();
    let reset = block.read().unwrap();
    assert!(reset.entities().is_empty());
    assert!(reset.pixels().iter().all(|color| *color == 7));
}
