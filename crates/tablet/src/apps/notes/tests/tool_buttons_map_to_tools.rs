use super::super::*;

#[test]
fn tool_buttons_map_to_tools() {
    assert_eq!(
        tool_at_position(Vector::new(OUTER_MARGIN + 8.0, STATUS_BAR_HEIGHT + 18.0)),
        Some(Tool::Pen)
    );
    assert_eq!(
        tool_at_position(Vector::new(
            OUTER_MARGIN + TOOL_BUTTON_WIDTH + TOOL_BUTTON_GAP + 8.0,
            STATUS_BAR_HEIGHT + 18.0
        )),
        Some(Tool::Highlighter)
    );
    assert_eq!(
        tool_at_position(Vector::new(
            OUTER_MARGIN + (TOOL_BUTTON_WIDTH + TOOL_BUTTON_GAP) * 2.0 + 8.0,
            STATUS_BAR_HEIGHT + 18.0
        )),
        Some(Tool::Eraser)
    );
}
