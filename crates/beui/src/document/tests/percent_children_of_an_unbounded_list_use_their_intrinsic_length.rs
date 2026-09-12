use super::*;
use crate::reactive::{ItemSize, Row};

const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn percent_children_of_an_unbounded_list_use_their_intrinsic_length() {
    let inner = NodeRef::new();
    let document = build({
        let inner = inner.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Column @node_ref=&inner spacing=0.0>
                        <Row @sizing=ItemSize::Percent(100.0) spacing=0.0>
                            <Padding horizontal=0.0 vertical={ITEM_HEIGHT / 2.0}>
                                <Spacer />
                            </Padding>
                        </Row>
                    </Column>
                </Column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(inner.get()).height(), ITEM_HEIGHT);
}
