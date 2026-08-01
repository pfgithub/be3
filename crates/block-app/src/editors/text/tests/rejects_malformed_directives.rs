use super::{parse_embeds, BLOCK_ID};

#[test]
fn rejects_malformed_directives() {
    let malformed = [
        "{{_BLOCKEDITOR:not-a-uuid:}}".to_owned(),
        format!("{{{{_BLOCKEDITOR:{BLOCK_ID}}}}}"),
        format!("{{{{_BLOCKEDITOR:{BLOCK_ID}:missing-close"),
        format!("{{{{_BLOCKEDITOR:{BLOCK_ID}:multi\nline}}}}"),
    ];

    for text in malformed {
        assert!(parse_embeds(text.as_bytes()).is_empty(), "{text}");
    }
}
