use super::*;

#[test]
fn source_revert_restores_position() {
    let mut source = Source::new("f.cvl2", "abc");
    let start = source.get_position();

    source.take();
    source.take();
    assert_eq!(source.current_index, 2);

    source.revert(&start);
    assert_eq!(source.current_index, start.idx);
    assert_eq!(source.current_line, start.lyn);
    assert_eq!(source.current_col, start.col);
    assert_eq!(source.filename, start.fyl);
    assert_eq!(source.peek(), Some('a'));
}
