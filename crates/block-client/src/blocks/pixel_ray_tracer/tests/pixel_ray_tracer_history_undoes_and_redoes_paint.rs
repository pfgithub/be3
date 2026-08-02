use uuid::Uuid;

use super::{PixelRayTracer, PixelRayTracerOperation, PixelUpdate, PIXEL_RAY_TRACER_BACKGROUND};
use crate::BlockClient;

#[test]
fn pixel_ray_tracer_history_undoes_and_redoes_paint() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(PixelRayTracer::new());
    let index = 3 * 128 + 2;

    block.operate(PixelRayTracerOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 2,
            y: 3,
            color_index: 12,
        }],
    });
    assert_eq!(block.read().unwrap().pixels()[index], 12);
    block.undo();
    assert_eq!(
        block.read().unwrap().pixels()[index],
        PIXEL_RAY_TRACER_BACKGROUND
    );
    block.redo();
    assert_eq!(block.read().unwrap().pixels()[index], 12);
}
