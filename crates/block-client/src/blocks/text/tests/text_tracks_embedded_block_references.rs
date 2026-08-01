use super::TextDocument;
use block::Block;
use uuid::Uuid;

#[test]
fn text_tracks_embedded_block_references() {
    let [first, second] = std::array::from_fn(|_| Uuid::new_v4());
    let document = TextDocument::from_bytes(format!(
        concat!(
            "inline {{{{_BLOCKEDITOR:{first}:opaque:settings}}}}\n",
            "{{{{_BLOCKEDITOR:{second}:}}}}\n",
            "duplicate {{{{_BLOCKEDITOR:{first}:}}}}\n",
            "{{{{_BLOCKEDITOR:not-a-uuid:}}}}\n",
            "{{{{_BLOCKEDITOR:{second}:multi\nline}}}}\n",
            "{{{{_BLOCKEDITOR:{second}}}}}"
        ),
        first = first,
        second = second,
    ));

    assert_eq!(document.references(), vec![first, second]);
}
