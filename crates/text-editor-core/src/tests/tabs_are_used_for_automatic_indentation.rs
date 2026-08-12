use super::*;

#[test]
fn tabs_are_used_for_automatic_indentation() {
    let mut tester = EditorTester::with_language(b"\titem", TextLanguage::Markdown);
    tester.set_cursor(tester.pos(5));

    tester.execute(EditorCommand::SetIndentation(TextIndentation::Tabs));
    tester.execute(EditorCommand::Newline);

    assert_eq!(tester.editor.indentation(), TextIndentation::Tabs);
    tester.expect_content(b"\titem\n\t|");
}
