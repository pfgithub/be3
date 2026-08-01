use super::{parse_embeds, BLOCK_ID};

#[test]
fn rejects_malformed_directives() {
    let malformed = [
        "https://blocks.pfg.pw/0/not-a-uuid".to_owned(),
        format!("https://blocks.pfg.pw/1/{BLOCK_ID}"),
        format!("https://blocks.pfg.pw/0/{BLOCK_ID}suffix"),
        format!("https://blocks.pfg.pw/0/{BLOCK_ID}/extra"),
        format!("{{{{_BLOCKEDITOR:{BLOCK_ID}:}}}}"),
    ];

    for text in malformed {
        assert!(parse_embeds(text.as_bytes()).is_empty(), "{text}");
    }
}
