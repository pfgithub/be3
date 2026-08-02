use super::*;

#[test]
fn pixel_ray_tracer_view_settings_preserve_lighting_cache() {
    let mut scene = PixelRayTracer::new();
    let lighting_revision = scene.lighting_revision();

    PixelRayTracer::apply_operation(
        &mut scene,
        &PixelRayTracerOperation::SetViewRaySettings {
            settings: RaySettings {
                ray_count: 100,
                step_distance: 1.0,
                maximum_steps: 200,
            },
        },
    );

    assert_eq!(scene.lighting_revision(), lighting_revision);
    assert_ne!(scene.revision(), 0);
}
