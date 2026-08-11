use super::*;

#[test]
fn toggle_collapse_at_is_independent_of_the_cursor() {
    let content = "# A\nbody a\n\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let b_start = content.find("# B").unwrap();

    // The cursor sits elsewhere; toggling by position still targets B.
    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::ToggleCollapseAt(tester.pos(b_start)));
    let sections = tester.editor.collapsible_sections();
    assert!(
        sections
            .iter()
            .find(|section| section.line_start == b_start)
            .unwrap()
            .collapsed
    );
    assert!(
        !sections
            .iter()
            .find(|section| section.line_start == 0)
            .unwrap()
            .collapsed
    );

    tester.execute(EditorCommand::ToggleCollapseAt(tester.pos(b_start)));
    let sections = tester.editor.collapsible_sections();
    assert!(
        !sections
            .iter()
            .find(|section| section.line_start == b_start)
            .unwrap()
            .collapsed
    );

    // Toggling a position on a non-collapsible line does nothing.
    tester.execute(EditorCommand::ToggleCollapseAt(
        tester.pos(content.find("body a").unwrap()),
    ));
    assert!(tester
        .editor
        .collapsible_sections()
        .iter()
        .all(|section| !section.collapsed));
}
