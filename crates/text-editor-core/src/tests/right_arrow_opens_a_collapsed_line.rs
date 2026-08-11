use super::*;

#[test]
fn right_arrow_opens_a_collapsed_line() {
    let content = "# A\nbody a1\nbody a2\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);

    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::Collapse);
    let section = tester.editor.collapsible_sections()[0];
    assert!(section.collapsed);

    tester.set_cursor(tester.pos(section.line_end));
    tester.execute(EditorCommand::MoveCursorLeftRight {
        direction: LRDirection::Right,
        mode: MoveMode::Move,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    assert!(!tester.editor.collapsible_sections()[0].collapsed);
    let cursor = tester.editor.cursor_positions()[0];
    let focus = tester.editor.position_index(cursor.pos.focus).unwrap();
    assert_eq!(focus, section.line_end);
}
