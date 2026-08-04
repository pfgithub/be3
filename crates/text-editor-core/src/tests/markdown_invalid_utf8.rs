use super::*;

#[test]
fn markdown_invalid_utf8() {
    let mut tester = EditorTester::with_language(b"**a\xffb**", TextLanguage::Markdown);
    let highlight = tester.editor.highlight();
    for index in 0..7 {
        let _ = highlight.style_at(index);
    }
    assert_eq!(highlight.style_at(7).color, SynHlColorScope::Invalid);
}
