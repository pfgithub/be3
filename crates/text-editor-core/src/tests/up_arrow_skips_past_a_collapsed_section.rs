use super::*;

#[test]
fn up_arrow_skips_past_a_collapsed_section() {
    let content = "# A\nbody a1\nbody a2\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let b_start = content.find("# B").unwrap();

    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::Collapse);
    assert!(tester.editor.collapsible_sections()[0].collapsed);

    tester.set_cursor(tester.pos(b_start));
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        mode: VerticalMoveMode::Move,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    let cursor = tester.editor.cursor_positions()[0];
    let focus = tester.editor.position_index(cursor.pos.focus).unwrap();
    assert_eq!(focus, 0);
}
