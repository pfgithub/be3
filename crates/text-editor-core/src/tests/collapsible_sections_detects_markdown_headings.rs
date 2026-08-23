use super::*;

#[test]
fn collapsible_sections_detects_markdown_headings() {
    let lines = [
        "# Title", "intro", "", "## A", "line a1", "line a2", "", "### Sub", "sub line", "",
        "## B", "line b", "", "# Second", "last",
    ];
    let content = lines.join("\n");
    let offset = |index: usize| -> usize { lines[..index].iter().map(|line| line.len() + 1).sum() };
    let tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);

    let sections = tester.editor.collapsible_sections();
    let starts = sections
        .iter()
        .map(|section| section.line_start)
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        vec![offset(0), offset(3), offset(7), offset(10), offset(13)]
    );
    assert!(sections.iter().all(|section| !section.collapsed));

    let title = sections[0];
    assert_eq!(title.line_end, offset(1) - 1);
    assert_eq!(title.content_end, offset(12) - 1);

    let section_a = sections[1];
    assert_eq!(section_a.line_end, offset(4) - 1);
    assert_eq!(section_a.content_end, offset(9) - 1);

    let subsection = sections[2];
    assert_eq!(subsection.line_end, offset(8) - 1);
    assert_eq!(subsection.content_end, offset(9) - 1);

    let section_b = sections[3];
    assert_eq!(section_b.line_end, offset(11) - 1);
    assert_eq!(section_b.content_end, offset(12) - 1);

    let second = sections[4];
    assert_eq!(second.line_end, offset(14) - 1);
    assert_eq!(second.content_end, content.len());
}
