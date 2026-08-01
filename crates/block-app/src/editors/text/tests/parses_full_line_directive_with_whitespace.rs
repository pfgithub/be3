use super::{directive, parse_embeds, BLOCK_ID};

#[test]
fn parses_full_line_directive_with_whitespace() {
    let directive = directive("");
    let text = format!("before\n \t{directive}\r \nafter");
    let start = text.find("{{").unwrap();

    let embeds = parse_embeds(text.as_bytes());

    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].id, BLOCK_ID);
    assert_eq!(embeds[0].range, start..start + directive.len());
    assert!(embeds[0].full_line);
}
