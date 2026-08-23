use super::*;

#[test]
fn markdown_inline_code_escapes_backtick_runs() {
    let mut tester = EditorTester::with_language(b"abc", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"`[abc|`");

    let mut tester = EditorTester::with_language(b"abc`def", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``[abc`def|``");

    let mut tester = EditorTester::with_language(b"a``b", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"```[a``b|```");
}
