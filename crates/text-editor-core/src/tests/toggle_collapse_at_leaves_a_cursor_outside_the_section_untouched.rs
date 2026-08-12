use super::*;

#[test]
fn toggle_collapse_at_leaves_a_cursor_outside_the_section_untouched() {
    let content = "# A\nbody a\n\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let a_start = content.find("# A").unwrap();
    let b_start = content.find("# B").unwrap();

    tester.set_cursor(tester.pos(b_start));
    tester.execute(EditorCommand::ToggleCollapseAt(tester.pos(a_start)));

    tester.expect_content("# A\nbody a\n\n|# B\nbody b\n");
}
