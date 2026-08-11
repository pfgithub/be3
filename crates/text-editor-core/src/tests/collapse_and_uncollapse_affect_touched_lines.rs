use super::*;

#[test]
fn collapse_and_uncollapse_affect_touched_lines() {
    let content = "# A\nbody a\n\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let a_start = 0;
    let b_start = content.find("# B").unwrap();

    // A cursor on a non-collapsible line collapses nothing.
    tester.set_cursor(tester.pos(content.find("body a").unwrap()));
    tester.execute(EditorCommand::Collapse);
    assert!(tester
        .editor
        .collapsible_sections()
        .iter()
        .all(|section| !section.collapsed));

    // A selection spanning both heading lines collapses both.
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(a_start),
        focus: tester.pos(b_start + "# B".len()),
    });
    tester.execute(EditorCommand::Collapse);
    let sections = tester.editor.collapsible_sections();
    assert!(
        sections
            .iter()
            .find(|section| section.line_start == a_start)
            .unwrap()
            .collapsed
    );
    assert!(
        sections
            .iter()
            .find(|section| section.line_start == b_start)
            .unwrap()
            .collapsed
    );

    // Collapsing an already-collapsed line is a no-op, and Uncollapse
    // reverses it.
    tester.execute(EditorCommand::Collapse);
    tester.set_cursor(tester.pos(a_start));
    tester.execute(EditorCommand::Uncollapse);
    let sections = tester.editor.collapsible_sections();
    assert!(
        !sections
            .iter()
            .find(|section| section.line_start == a_start)
            .unwrap()
            .collapsed
    );
    assert!(
        sections
            .iter()
            .find(|section| section.line_start == b_start)
            .unwrap()
            .collapsed
    );
}
