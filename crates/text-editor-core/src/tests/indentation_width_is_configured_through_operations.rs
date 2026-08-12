use super::*;

#[test]
fn indentation_width_is_configured_through_operations() {
    let mut tester = EditorTester::with_language(b"   item", TextLanguage::Markdown);
    tester.set_cursor(tester.pos(7));

    tester.execute(EditorCommand::SetIndentation(TextIndentation::Spaces {
        width: 3,
    }));
    tester.execute(EditorCommand::Newline);

    assert_eq!(
        tester.editor.indentation(),
        TextIndentation::Spaces { width: 3 }
    );
    tester.expect_content(b"   item\n   |");
}
