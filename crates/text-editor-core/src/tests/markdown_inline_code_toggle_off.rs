use super::*;

#[test]
fn markdown_inline_code_toggle_off() {
    let mut tester = EditorTester::with_language(b"`hi``", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");

    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"[`hi``|");

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"[`hi``|");
}
