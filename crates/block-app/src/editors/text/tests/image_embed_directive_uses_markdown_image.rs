use super::{block_url, image_embed_directive, BLOCK_ID};

#[test]
fn image_embed_directive_uses_markdown_image() {
    assert_eq!(
        image_embed_directive(BLOCK_ID, "Pasted Image.png", true),
        format!("![Pasted Image.png]({})", block_url(BLOCK_ID))
    );
}
