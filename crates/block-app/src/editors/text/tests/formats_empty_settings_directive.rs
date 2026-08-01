use super::{format_embed, BLOCK_ID};

#[test]
fn formats_empty_settings_directive() {
    assert_eq!(
        format_embed(BLOCK_ID),
        format!("{{{{_BLOCKEDITOR:{BLOCK_ID}:}}}}")
    );
}
