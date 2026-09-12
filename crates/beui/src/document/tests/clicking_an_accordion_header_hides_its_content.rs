use super::*;
use crate::reactive::{view, NodeRef, TextBuilder};
use crate::styled::AccordionBuilder;

#[test]
fn clicking_an_accordion_header_hides_its_content() {
    let body = NodeRef::new();
    let (document, [accordion]) = toolbar_of({
        let body = body.clone();
        move || {
            [view! {
                <accordion title="About" open=true>
                    <text
                        node_ref=&body
                        string="beui keeps a retained tree of nodes."
                        font_size=14.0
                        color=Color32::WHITE
                    />
                </accordion>
            }]
        }
    });
    let body = body.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert!(harness.document().node_rect(body).is_some());

    let header = harness.rect(accordion);
    harness.click(pos2(header.center().x, header.top() + 8.0));
    harness.frame(Vec::new());

    assert!(!styled::accordion_open(harness.document(), accordion));
    assert!(harness.document().node_rect(body).is_none());
}
