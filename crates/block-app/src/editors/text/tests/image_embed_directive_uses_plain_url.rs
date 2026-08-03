use super::{block_url, image_embed_directive, BLOCK_ID, WORKSPACE_ID};

#[test]
fn image_embed_directive_uses_plain_url() {
    assert_eq!(
        image_embed_directive(WORKSPACE_ID, BLOCK_ID, "Pasted Image.png", false),
        block_url(WORKSPACE_ID, BLOCK_ID)
    );
}
