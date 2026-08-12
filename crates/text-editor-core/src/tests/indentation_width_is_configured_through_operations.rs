use super::*;

#[test]
fn indentation_width_is_configured_through_operations() {
    let mut tester = EditorTester::with_language(b"   item", TextLanguage::Markdown);
    tester.set_cursor(tester.pos(7));

    tester.execute(EditorCommand::SetIndentWidth(3));
    tester.execute(EditorCommand::Newline);

    assert_eq!(tester.editor.indent_width(), 3);
    tester.expect_content(b"   item\n   |");
}
