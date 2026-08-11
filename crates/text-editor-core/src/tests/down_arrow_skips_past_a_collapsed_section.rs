use super::*;

#[test]
fn down_arrow_skips_past_a_collapsed_section() {
    let content = "# A\nbody a1\nbody a2\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let b_start = content.find("# B").unwrap();

    tester.set_cursor(tester.pos(1));
    tester.execute(EditorCommand::Collapse);
    assert!(tester.editor.collapsible_sections()[0].collapsed);

    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Move,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    let cursor = tester.editor.cursor_positions()[0];
    let focus = tester.editor.position_index(cursor.pos.focus).unwrap();
    assert_eq!(focus, b_start + 1);
}
