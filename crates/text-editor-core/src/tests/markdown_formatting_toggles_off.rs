use super::*;

#[test]
fn markdown_formatting_toggles_off() {
    let mut tester = EditorTester::with_language(b"hello", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
    tester.expect_content(b"**[hello|**");

    tester.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
    tester.expect_content(b"[hello|");

    tester.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
    tester.expect_content(b"_[hello|_");
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
    tester.expect_content(b"[hello|");
}
