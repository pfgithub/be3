use uuid::Uuid;

use super::{PixelRayTracer, PixelRayTracerOperation, RaySettings};
use crate::BlockClient;

#[test]
fn pixel_ray_tracer_history_undoes_and_redoes_ray_settings() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(PixelRayTracer::new());
    let original = block.read().unwrap().view_ray_settings();
    let changed = RaySettings {
        ray_count: 123,
        step_distance: 1.25,
        maximum_steps: 456,
    };

    block.operate(PixelRayTracerOperation::SetViewRaySettings { settings: changed });
    block.undo();
    assert_eq!(block.read().unwrap().view_ray_settings(), original);
    block.redo();
    assert_eq!(block.read().unwrap().view_ray_settings(), changed);
}
