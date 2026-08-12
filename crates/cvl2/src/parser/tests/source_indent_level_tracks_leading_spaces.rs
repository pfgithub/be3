use super::*;

#[test]
fn source_indent_level_tracks_leading_spaces() {
    let source = Source::new("f.cvl2", "  abc");
    assert_eq!(source.current_line_indent_level, 2);

    let mut source_with_newline = Source::new("f.cvl2", "a\n    b");
    assert_eq!(source_with_newline.current_line_indent_level, 0);

    source_with_newline.take();
    source_with_newline.take();
    assert_eq!(source_with_newline.current_line_indent_level, 4);
}
