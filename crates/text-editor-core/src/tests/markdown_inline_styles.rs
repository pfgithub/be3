use super::*;

#[test]
fn markdown_inline_styles() {
    let mut tester = EditorTester::with_language(
        b"Hello **bold** *italic* ***both*** ~~gone~~ `code` [link](target)",
        Language::Markdown,
    );
    let highlight = tester.editor.highlight();

    assert_eq!(
        highlight.style_at(0).color,
        SynHlColorScope::MarkdownPlainText
    );
    assert_eq!(highlight.style_at(6).color, SynHlColorScope::MarkdownSymbol);
    assert!(highlight.style_at(8).bold);
    assert_eq!(
        highlight.style_at(12).color,
        SynHlColorScope::MarkdownSymbol
    );
    assert!(highlight.style_at(17).italic);
    assert!(highlight.style_at(28).bold);
    assert!(highlight.style_at(28).italic);
    assert!(highlight.style_at(39).strikethrough);
    assert_eq!(highlight.style_at(47).family, SynHlFontFamily::Monospace);
    assert_eq!(highlight.style_at(47).color, SynHlColorScope::MarkdownCode);
    assert_eq!(highlight.style_at(55).color, SynHlColorScope::MarkdownLink);
    assert!(highlight.style_at(55).underline);
    assert_eq!(highlight.style_at(61).family, SynHlFontFamily::Monospace);
}
