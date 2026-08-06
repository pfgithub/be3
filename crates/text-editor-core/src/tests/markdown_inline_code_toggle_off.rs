use super::*;

/// Applying inline code again to already-escaped content toggles it back off
/// and strips any padding space that was added, whether the selection sits
/// just inside the delimiters or spans the delimiters themselves.
#[test]
fn markdown_inline_code_toggle_off() {
    let mut tester = EditorTester::with_language(b"`hi``", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");

    // Applying it again to the same (inner) selection unwraps instead of doubling up.
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"[`hi``|");

    // Selecting the delimiters (and padding) themselves also toggles off.
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"[`hi``|");
}
