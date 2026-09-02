use super::{block_url, parse_embeds, BLOCK_ID, WORKSPACE_ID};
use uuid::Uuid;

#[test]
fn foreign_workspace_url_is_not_an_embed() {
    let url = block_url(Uuid::new_v4(), BLOCK_ID);
    assert!(parse_embeds(url.as_bytes(), WORKSPACE_ID, true).is_empty());
}
