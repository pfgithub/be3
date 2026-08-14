use crate::block_ref::BlockRef;
use crate::{parse_block_urls, repo_relative_block_url};
use uuid::Uuid;

#[test]
fn block_urls_parse_repo_relative_references() {
    let repo = Uuid::new_v4();
    let eternal_id = Uuid::new_v4();
    let url = repo_relative_block_url(repo, eternal_id);

    let parsed = parse_block_urls(url.as_bytes());

    assert_eq!(parsed.len(), 1);
    assert_eq!(
        parsed[0].reference,
        BlockRef::RepoRelative { repo, eternal_id }
    );
    assert_eq!(parsed[0].workspace_id, None);
    assert_eq!(parsed[0].range, 0..url.len());
}
