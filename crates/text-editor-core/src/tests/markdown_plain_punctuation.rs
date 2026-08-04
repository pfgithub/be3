use super::*;

#[test]
fn markdown_plain_punctuation() {
    let source = b"Hello! (ordinary) | pipe ~ tilde\n[link](target) ![image](source)";
    let mut tester = EditorTester::with_language(source, TextLanguage::Markdown);
    let highlight = tester.editor.highlight();

    for index in [5, 7, 16, 18, 25] {
        assert_eq!(
            highlight.style_at(index).color,
            SynHlColorScope::MarkdownPlainText
        );
    }

    for index in [33, 38, 39, 46, 48, 49, 55, 56, 63] {
        assert_eq!(
            highlight.style_at(index).color,
            SynHlColorScope::MarkdownSymbol
        );
    }
}
