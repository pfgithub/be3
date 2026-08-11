use super::*;

#[test]
fn collapsible_sections_detects_indentation_blocks() {
    let lines = [
        "outer",          // 0
        "    inner",      // 1
        "        nested", // 2
        "",               // 3
        "    inner2",     // 4
        "trailing",       // 5
    ];
    let content = lines.join("\n");
    let offset = |index: usize| -> usize { lines[..index].iter().map(|line| line.len() + 1).sum() };
    let tester = EditorTester::with_language(content.as_bytes(), TextLanguage::PlainText);

    let sections = tester.editor.collapsible_sections();
    let starts = sections
        .iter()
        .map(|section| section.line_start)
        .collect::<Vec<_>>();
    // "nested", "inner2" and "trailing" have no more-indented following line,
    // so they aren't collapsible.
    assert_eq!(starts, vec![offset(0), offset(1)]);

    let outer = sections[0];
    assert_eq!(outer.line_end, offset(1) - 1);
    assert_eq!(outer.content_end, offset(5) - 1);

    let inner = sections[1];
    assert_eq!(inner.line_end, offset(2) - 1);
    assert_eq!(inner.content_end, offset(3) - 1);
}
