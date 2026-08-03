use crate::{block_url, block_url_prefix, parse_block_urls};
use uuid::Uuid;

#[test]
fn block_urls_include_workspace_and_reject_malformed_paths() {
    let workspace_id = Uuid::new_v4();
    let block_id = Uuid::new_v4();
    let url = block_url(workspace_id, block_id);
    assert_eq!(url, format!("{}{block_id}", block_url_prefix(workspace_id)));
    let parsed = parse_block_urls(url.as_bytes());
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].workspace_id, workspace_id);
    assert_eq!(parsed[0].id, block_id);
    assert_eq!(parsed[0].range, 0..url.len());

    for malformed in [
        format!("https://blocks.pfg.pw/{block_id}"),
        format!("https://blocks.pfg.pw/not-a-uuid/{block_id}"),
        format!("https://blocks.pfg.pw/{workspace_id}/not-a-uuid"),
        format!("{url}/extra"),
    ] {
        assert!(parse_block_urls(malformed.as_bytes()).is_empty());
    }
}
