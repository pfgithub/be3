use super::*;

#[test]
fn block_urls_have_no_cursor_stops_inside_them() {
    let url = block_url(Uuid::from_u128(1), Uuid::from_u128(2));
    let text = format!("a {url} b");
    let bytes = text.as_bytes();
    let start = 2;
    let end = start + url.len();

    assert!(!inside_block_url(bytes, start));
    assert!(!inside_block_url(bytes, end));
    for index in start + 1..end {
        assert!(inside_block_url(bytes, index), "index {index} has a stop");
    }
}
