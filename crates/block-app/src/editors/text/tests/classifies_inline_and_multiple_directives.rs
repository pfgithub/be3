use super::{directive, parse_embeds};

#[test]
fn classifies_inline_and_multiple_directives() {
    let first = directive("first");
    let second = directive("second");
    let text = format!("left {first} middle {second} right");

    let embeds = parse_embeds(text.as_bytes());

    assert_eq!(embeds.len(), 2);
    assert!(embeds.iter().all(|embed| !embed.full_line));
    assert_eq!(&text.as_bytes()[embeds[0].settings.clone()], b"first");
    assert_eq!(&text.as_bytes()[embeds[1].settings.clone()], b"second");
}
