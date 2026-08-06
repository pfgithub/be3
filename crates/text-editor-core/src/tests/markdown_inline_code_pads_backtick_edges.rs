use super::*;

/// When the chosen delimiter is a multi-backtick run, a selection that
/// starts or ends with a backtick needs a padding space on that edge so the
/// delimiter doesn't visually merge with the content. Each edge is padded
/// independently.
#[test]
fn markdown_inline_code_pads_backtick_edges() {
    // Starts with a backtick, doesn't end with one: pad the leading edge only.
    let mut tester = EditorTester::with_language(b"`ab", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"`` [`ab|``");

    // Ends with a backtick, doesn't start with one: pad the trailing edge only.
    let mut tester = EditorTester::with_language(b"ab`", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``[ab`| ``");

    // Starts and ends with a backtick: pad both edges. (`hi`` also contains a
    // run of two backticks, so the delimiter is three backticks.)
    let mut tester = EditorTester::with_language(b"`hi``", TextLanguage::Markdown);
    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::InlineCode));
    tester.expect_content(b"``` [`hi``| ```");
}
