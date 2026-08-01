use super::{block_url, parse_embeds, BLOCK_ID};

#[test]
fn parses_full_line_directive_with_whitespace() {
    let url = block_url();
    let text = format!("before\n \t{url}\r \nafter");
    let start = text.find("https://").unwrap();

    let embeds = parse_embeds(text.as_bytes());

    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].id, BLOCK_ID);
    assert_eq!(embeds[0].range, start..start + url.len());
    assert!(embeds[0].full_line);
}
