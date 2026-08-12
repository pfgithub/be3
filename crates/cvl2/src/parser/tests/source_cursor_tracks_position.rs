use super::*;

#[test]
fn source_cursor_tracks_position() {
    let mut source = Source::new("f.cvl2", "ab\ncd");

    assert_eq!(source.peek(), Some('a'));
    assert_eq!(source.current_index, 0);

    assert_eq!(source.take(), Some('a'));
    assert_eq!(source.current_index, 1);
    assert_eq!(source.current_line, 1);
    assert_eq!(source.current_col, 2);

    assert_eq!(source.take(), Some('b'));
    assert_eq!(source.current_col, 3);

    assert_eq!(source.take(), Some('\n'));
    assert_eq!(source.current_line, 2);
    assert_eq!(source.current_col, 1);

    assert_eq!(source.take(), Some('c'));
    assert_eq!(source.take(), Some('d'));
    assert_eq!(source.peek(), None);
    assert_eq!(source.take(), None);
    assert_eq!(source.current_index, 5);
}
