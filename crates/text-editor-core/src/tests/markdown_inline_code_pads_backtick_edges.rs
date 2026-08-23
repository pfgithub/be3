use super::*;

#[test]
fn markdown_inline_code_pads_backtick_edges() {
    let mut tester = EditorTester::with_language(b"`ab", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"`` [`ab|``");

    let mut tester = EditorTester::with_language(b"ab`", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``[ab`| ``");

    let mut tester = EditorTester::with_language(b"`hi``", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");
}
