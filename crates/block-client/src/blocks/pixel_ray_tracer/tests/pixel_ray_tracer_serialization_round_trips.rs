use super::*;

#[test]
fn pixel_ray_tracer_serialization_round_trips() {
    let mut scene = PixelRayTracer::new();
    PixelRayTracer::apply_operation(
        &mut scene,
        &PixelRayTracerOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 12,
                y: 34,
                color_index: 8,
            }],
        },
    );
    PixelRayTracer::apply_operation(
        &mut scene,
        &PixelRayTracerOperation::AddEntity {
            entity: RayEntity::Surface {
                id: 1,
                start: Point::new(10.0, 20.0),
                end: Point::new(70.0, 80.0),
                color_index: 12,
                roughness: 0.25,
                metalness: 0.5,
                transmission: 0.75,
                refractive_index: 1.5,
            },
        },
    );

    let serialized = serde_json::to_vec(&scene).unwrap();
    let deserialized: PixelRayTracer = serde_json::from_slice(&serialized).unwrap();

    assert_eq!(deserialized, scene);
}
