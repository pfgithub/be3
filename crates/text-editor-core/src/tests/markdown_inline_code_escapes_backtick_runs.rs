use super::*;

/// The inline-code delimiter must be one backtick longer than the longest
/// run of backticks already inside the selection, so the escaped span can't
/// be mistaken for ending early.
#[test]
fn markdown_inline_code_escapes_backtick_runs() {
    // No backticks in the selection: a single backtick delimiter, same as before.
    let mut tester = EditorTester::with_language(b"abc", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"`[abc|`");

    // A single backtick inside the selection: needs a two-backtick delimiter.
    let mut tester = EditorTester::with_language(b"abc`def", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``[abc`def|``");

    // A run of two backticks inside the selection: needs a three-backtick delimiter.
    let mut tester = EditorTester::with_language(b"a``b", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"```[a``b|```");
}
