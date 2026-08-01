use super::*;

#[test]
fn markdown_checkbox_toggle() {
    let mut tester = EditorTester::with_language(b"- [ ] task", Language::Markdown);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
        tester.pos(0),
    )));
    tester.expect_content(b"- [x] task");

    tester.execute(EditorCommand::SetCursorPosition(Position::END));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- [x] task\n- [ ] |");
}
