use super::TextDocument;
use crate::{block_url, block_url_prefix};
use block::Block;
use uuid::Uuid;

#[test]
fn text_tracks_embedded_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let workspace = Uuid::new_v4();
    let first_url = block_url(workspace, first);
    let second_url = block_url(workspace, second);
    let prefix = block_url_prefix(workspace);
    let document = TextDocument::from_bytes(format!(
        "inline {first_url}\n{second_url}\nduplicate {first_url}\n{prefix}not-a-uuid\nhttps://blocks.pfg.pw/1/{second}\n{second_url}/extra\n{{{{_BLOCKEDITOR:{second}:}}}}"
    ));

    assert_eq!(document.references(), vec![first, second]);
    assert_eq!(
        document.references_for_workspace(workspace),
        vec![first, second]
    );
    assert!(document.references_for_workspace(Uuid::new_v4()).is_empty());
}
