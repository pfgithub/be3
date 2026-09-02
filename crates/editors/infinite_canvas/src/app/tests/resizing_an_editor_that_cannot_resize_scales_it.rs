use super::*;

#[test]
fn resizing_an_editor_that_cannot_resize_scales_it() {
    let block_id = BlockRef::Direct(Uuid::new_v4());
    let mut editor = entity(Uuid::new_v4());
    editor.kind = CanvasEntityKind::DirectEditor {
        block_id,
        scale: 1.0,
    };
    editor.transform = CanvasTransform::new(
        CanvasPoint::new(50.0, 50.0),
        CanvasPoint::new(100.0, 100.0),
        0.0,
    );
    let bounds = entity_bounds(&editor);

    let scaled = resize_entities_axis(
        ResizeHandle { x: 1, y: 1 },
        bounds,
        CanvasPoint::new(bounds.min.x + 200.0, bounds.min.y + 200.0),
        std::slice::from_ref(&editor),
        true,
        false,
        true,
    );
    let resized = resize_entities_axis(
        ResizeHandle { x: 1, y: 1 },
        bounds,
        CanvasPoint::new(bounds.min.x + 200.0, bounds.min.y + 200.0),
        std::slice::from_ref(&editor),
        true,
        false,
        false,
    );

    let CanvasEntityKind::DirectEditor { scale, .. } = scaled[0].kind else {
        panic!("the entity is no longer a direct editor");
    };
    assert!((scale - 2.0).abs() < 0.001);
    assert!((scaled[0].transform.size.x - 200.0).abs() < 0.001);
    let CanvasEntityKind::DirectEditor { scale, .. } = resized[0].kind else {
        panic!("the entity is no longer a direct editor");
    };
    assert!((scale - 1.0).abs() < 0.001);
    assert!((resized[0].transform.size.x - 200.0).abs() < 0.001);
}
