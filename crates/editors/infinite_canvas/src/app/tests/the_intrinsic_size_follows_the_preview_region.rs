use super::{editor, CanvasPoint, CanvasPreviewRegion, InfiniteCanvasOperation};
use block_editor_plugin::{egui, App as _};

#[test]
fn the_intrinsic_size_follows_the_preview_region() {
    let (mut editor, block) = editor(&[]);
    block.operate(InfiniteCanvasOperation::SetPreviewRegion {
        region: Some(CanvasPreviewRegion::new(
            CanvasPoint::default(),
            CanvasPoint::new(960.0, 540.0),
        )),
    });

    assert_eq!(
        editor.app().intrinsic_size(),
        Some(egui::vec2(960.0, 540.0))
    );

    editor.app().set_intrinsic_size(egui::vec2(480.0, 270.0));

    assert_eq!(
        editor.app().intrinsic_size(),
        Some(egui::vec2(480.0, 270.0))
    );
}
