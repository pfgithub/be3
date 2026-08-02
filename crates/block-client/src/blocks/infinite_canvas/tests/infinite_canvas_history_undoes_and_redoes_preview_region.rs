use uuid::Uuid;

use super::{CanvasPoint, CanvasPreviewRegion, InfiniteCanvas, InfiniteCanvasOperation};
use crate::BlockClient;

#[test]
fn infinite_canvas_history_undoes_and_redoes_preview_region() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(InfiniteCanvas::new());
    let region =
        CanvasPreviewRegion::new(CanvasPoint::new(20.0, 30.0), CanvasPoint::new(400.0, 240.0));
    block.operate(InfiniteCanvasOperation::SetPreviewRegion {
        region: Some(region),
    });
    assert_eq!(block.read().unwrap().preview_region(), Some(region));

    block.undo();
    assert_eq!(block.read().unwrap().preview_region(), None);
    block.redo();
    assert_eq!(block.read().unwrap().preview_region(), Some(region));
}
