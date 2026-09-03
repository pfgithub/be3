use super::*;

#[test]
fn reports_cursor_position() {
    let screen = render(20, 3, "abc");

    let cursor = screen.cursor.expect("the cursor is visible by default");
    assert_eq!((cursor.x, cursor.y), (3, 0));
}
