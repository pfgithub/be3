use super::{block_url, image_embed_directive, BlockRef, BLOCK_ID, WORKSPACE_ID};

#[test]
fn image_embed_directive_uses_markdown_image() {
    assert_eq!(
        image_embed_directive(
            WORKSPACE_ID,
            &BlockRef::Direct(BLOCK_ID),
            "Pasted Image.png",
            true
        ),
        format!("![Pasted Image.png]({})", block_url(WORKSPACE_ID, BLOCK_ID))
    );
}
