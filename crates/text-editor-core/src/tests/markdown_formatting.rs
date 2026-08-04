use super::*;

#[test]
fn markdown_formatting() {
    let mut tester = EditorTester::with_language(b"hello", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
    tester.expect_content(b"**[hello|**");

    tester.execute(EditorCommand::SetCursorPosition(tester.pos(9)));
    tester.execute(EditorCommand::Markdown(MarkdownCommand::Link));
    tester.expect_content(b"**hello**[[link text|](url)");
}
