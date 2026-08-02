use block::Block;

use super::{CanvasPoint, CanvasPreviewRegion, InfiniteCanvas, InfiniteCanvasOperation};

#[test]
fn infinite_canvas_sets_preview_region() {
    let mut canvas = InfiniteCanvas::new();
    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(CanvasPreviewRegion::new(
                CanvasPoint::new(20.0, 30.0),
                CanvasPoint::new(-400.0, 240.0),
            )),
        },
    );
    assert_eq!(
        canvas.preview_region(),
        Some(CanvasPreviewRegion::new(
            CanvasPoint::new(20.0, 30.0),
            CanvasPoint::new(400.0, 240.0),
        ))
    );

    InfiniteCanvas::apply_operation(
        &mut canvas,
        &InfiniteCanvasOperation::SetPreviewRegion { region: None },
    );
    assert_eq!(canvas.preview_region(), None);
}
