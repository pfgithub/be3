use super::*;

#[test]
fn markdown_block_styles() {
    let source = b"# Heading\n> quote\n- [x] task\n\n| Head |\n| --- |\n| cell |\n\n```zig\nconst x = 1;\n```\n";
    let mut tester = EditorTester::with_language(source, Language::Markdown);
    let highlight = tester.editor.highlight();

    assert_eq!(highlight.style_at(0).color, SynHlColorScope::MarkdownSymbol);
    assert_eq!(highlight.style_at(2).size, SynHlTextSize::Heading(1));
    assert!(highlight.style_at(2).bold);
    assert_eq!(
        highlight.style_at(10).color,
        SynHlColorScope::MarkdownSymbol
    );
    assert!(highlight.style_at(12).italic);
    assert_eq!(
        highlight.style_at(18).color,
        SynHlColorScope::MarkdownSymbol
    );
    assert_eq!(
        highlight.style_at(20).color,
        SynHlColorScope::MarkdownSymbol
    );
    assert!(highlight.style_at(35).bold);
    assert_eq!(
        highlight.style_at(57).color,
        SynHlColorScope::MarkdownSymbol
    );
    assert_eq!(highlight.style_at(60).family, SynHlFontFamily::Monospace);
    assert_eq!(highlight.style_at(64).family, SynHlFontFamily::Monospace);
    assert_eq!(highlight.style_at(64).color, SynHlColorScope::MarkdownCode);
}
