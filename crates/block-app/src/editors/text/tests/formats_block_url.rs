use super::{format_embed, BLOCK_ID};

#[test]
fn formats_block_url() {
    assert_eq!(
        format_embed(BLOCK_ID),
        format!("https://blocks.pfg.pw/0/{BLOCK_ID}")
    );
}
