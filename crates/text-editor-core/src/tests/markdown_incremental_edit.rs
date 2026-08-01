use super::*;

#[test]
fn markdown_incremental_edit() {
    let mut tester = EditorTester::with_language(b"plain", Language::Markdown);
    assert!(!tester.editor.highlight().style_at(0).bold);

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::InsertText(b"**bold**"));
    let highlight = tester.editor.highlight();
    assert_eq!(highlight.style_at(0).color, SynHlColorScope::MarkdownSymbol);
    assert!(highlight.style_at(2).bold);
    assert_eq!(highlight.style_at(6).color, SynHlColorScope::MarkdownSymbol);
}
