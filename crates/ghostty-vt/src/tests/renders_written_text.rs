use super::*;

#[test]
fn renders_written_text() {
    let screen = render(20, 3, "hello\r\nworld");

    assert_eq!(screen.rows.len(), 3);
    assert_eq!(screen.text(), "hello\nworld\n");
    assert_eq!(screen.rows[0].cells[0].text, "h");
}
