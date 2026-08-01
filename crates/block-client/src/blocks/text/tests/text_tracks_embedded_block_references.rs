use super::TextDocument;
use crate::{block_url, BLOCK_URL_PREFIX};
use block::Block;
use uuid::Uuid;

#[test]
fn text_tracks_embedded_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let first_url = block_url(first);
    let second_url = block_url(second);
    let document = TextDocument::from_bytes(format!(
        "inline {first_url}\n{second_url}\nduplicate {first_url}\n{BLOCK_URL_PREFIX}not-a-uuid\nhttps://blocks.pfg.pw/1/{second}\n{second_url}/extra\n{{{{_BLOCKEDITOR:{second}:}}}}"
    ));

    assert_eq!(document.references(), vec![first, second]);
}
