use super::TextDocument;
use block::Block;
use uuid::Uuid;

#[test]
fn text_tracks_embedded_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let document = TextDocument::from_bytes(format!(
        concat!(
            "inline https://blocks.pfg.pw/0/{first}\n",
            "https://blocks.pfg.pw/0/{second}\n",
            "duplicate https://blocks.pfg.pw/0/{first}\n",
            "https://blocks.pfg.pw/0/not-a-uuid\n",
            "https://blocks.pfg.pw/1/{second}\n",
            "https://blocks.pfg.pw/0/{second}/extra\n",
            "{{{{_BLOCKEDITOR:{second}:}}}}"
        ),
        first = first,
        second = second,
    ));

    assert_eq!(document.references(), vec![first, second]);
}
