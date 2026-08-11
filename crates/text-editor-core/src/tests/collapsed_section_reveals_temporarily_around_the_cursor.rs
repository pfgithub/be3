use super::*;

#[test]
fn collapsed_section_reveals_temporarily_around_the_cursor() {
    let content = "# A\nbody a1\nbody a2\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);

    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::Collapse);
    assert!(tester.editor.collapsible_sections()[0].collapsed);

    // Find landing a match inside the hidden content reveals it...
    tester.execute(EditorCommand::Find {
        text: "a2",
        case_sensitive: false,
        direction: FindDirection::Next,
    });
    assert!(!tester.editor.collapsible_sections()[0].collapsed);

    // ...and it collapses again once the cursor moves away.
    tester.set_cursor(tester.pos(0));
    assert!(tester.editor.collapsible_sections()[0].collapsed);
}
