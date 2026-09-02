use super::{block_url, parse_embeds, BLOCK_ID, WORKSPACE_ID};

#[test]
fn classifies_markdown_image() {
    let url = block_url(WORKSPACE_ID, BLOCK_ID);
    let markdown_image = format!("![link]({url})");

    let image = parse_embeds(markdown_image.as_bytes(), WORKSPACE_ID, true);
    let plain = parse_embeds(url.as_bytes(), WORKSPACE_ID, true);

    assert_eq!(image.len(), 1);
    assert_eq!(image[0].range, 8..8 + url.len());
    assert!(image[0].large);
    assert_eq!(plain.len(), 1);
    assert!(!plain[0].large);
}
