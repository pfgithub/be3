use super::*;

#[test]
fn toggle_collapse_at_moves_the_cursor_out_of_a_newly_hidden_section() {
    let content = "# A\nbody a\n\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let a_start = content.find("# A").unwrap();
    let inside_body_a = content.find("body a").unwrap();

    tester.set_cursor(tester.pos(inside_body_a));
    tester.execute(EditorCommand::ToggleCollapseAt(tester.pos(a_start)));

    assert!(
        tester
            .editor
            .collapsible_sections()
            .iter()
            .find(|section| section.line_start == a_start)
            .unwrap()
            .collapsed
    );
    tester.expect_content("|# A\nbody a\n\n# B\nbody b\n");
}
