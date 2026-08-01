use super::{block_url, parse_embeds};

#[test]
fn classifies_inline_and_multiple_directives() {
    let first = block_url();
    let second = block_url();
    let text = format!("left {first} middle {second} right");

    let embeds = parse_embeds(text.as_bytes());

    assert_eq!(embeds.len(), 2);
    assert!(embeds.iter().all(|embed| !embed.full_line));
}
