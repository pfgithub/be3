use super::*;

#[test]
fn packed_previews_fill_rows() {
    let request = |child: u64, width: u32, height: u32| PreviewRequest {
        instance: EditorInstanceId(1),
        region: EditorRegion::LeftSidebar,
        child: ChildId(child),
        width,
        height,
    };
    let layout = PreviewLayout::packed(
        &[
            request(1, MAX_PREVIEW_EDGE - 64, 100),
            request(2, 128, 200),
            request(3, 64, 50),
        ],
        2.0,
    );
    assert_eq!(layout.slots.len(), 3);
    assert_eq!(
        (layout.slots[0].x, layout.slots[0].y),
        (0, 0),
        "the first preview starts the first row"
    );
    assert_eq!(
        (layout.slots[1].x, layout.slots[1].y),
        (0, 100),
        "a preview that does not fit starts the next row"
    );
    assert_eq!(
        (layout.slots[2].x, layout.slots[2].y),
        (128, 100),
        "the next preview shares that row"
    );
    assert_eq!(layout.width, MAX_PREVIEW_EDGE - 64);
    assert_eq!(layout.height, 300);
    assert_eq!(layout.scale_factor_millis, 2000);
}
