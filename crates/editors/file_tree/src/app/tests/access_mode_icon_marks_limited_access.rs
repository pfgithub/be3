use super::*;

#[test]
fn access_mode_icon_marks_limited_access() {
    assert_eq!(access_mode_icon(BlockAccess::Edit), None);
    assert_eq!(
        access_mode_icon(BlockAccess::View),
        Some(ICON_VISIBILITY.codepoint)
    );
    assert_eq!(
        access_mode_icon(BlockAccess::KnowExists),
        Some(ICON_LOCK.codepoint)
    );
    assert_eq!(
        access_mode_icon(BlockAccess::None),
        Some(ICON_LOCK.codepoint)
    );
}
