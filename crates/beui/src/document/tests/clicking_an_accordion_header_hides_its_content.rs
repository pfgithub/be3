use super::*;

#[test]
fn clicking_an_accordion_header_hides_its_content() {
    let mut document = Document::new();
    let body = styled::body(&mut document, "beui keeps a retained tree of nodes.");
    let accordion = styled::accordion(&mut document, "About", body, true);
    toolbar(&mut document, &[accordion]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert!(harness.document().node_rect(body).is_some());

    let header = harness.rect(accordion);
    harness.click(pos2(header.center().x, header.top() + 8.0));
    harness.frame(Vec::new());

    assert!(!styled::accordion_open(harness.document(), accordion));
    assert!(harness.document().node_rect(body).is_none());
}
