use super::{directive, parse_embeds};

#[test]
fn preserves_opaque_settings() {
    let text = directive("layout:wide:future=value");

    let embeds = parse_embeds(text.as_bytes());

    assert_eq!(embeds.len(), 1);
    assert_eq!(
        &text.as_bytes()[embeds[0].settings.clone()],
        b"layout:wide:future=value"
    );
}
