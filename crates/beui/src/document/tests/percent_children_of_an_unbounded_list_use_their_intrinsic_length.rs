use super::*;
use crate::reactive::RowBuilder;

const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn percent_children_of_an_unbounded_list_use_their_intrinsic_length() {
    let inner = NodeRef::new();
    let document = build({
        let inner = inner.clone();
        move || {
            view! {
                <column spacing={0.0}>
                    <column node_ref={&inner} spacing={0.0}>
                        @percent(100.0) <row spacing={0.0}>
                            <padding horizontal={0.0} vertical={ITEM_HEIGHT / 2.0}>
                                <spacer />
                            </padding>
                        </row>
                    </column>
                </column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(inner.get()).height(), ITEM_HEIGHT);
}
