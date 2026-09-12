use super::*;
use crate::reactive::{build, view, ItemSize};
use crate::styled::StackBuilder;
use crate::unstyled::ContainerBuilder;

const BREAKPOINT: f32 = 500.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_stack_becomes_a_column_when_its_container_gets_narrow() {
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (left, right) = (left.clone(), right.clone());
        move || {
            view! {
                <container>
                    {move |_| view! {
                        <stack spacing=0.0 breakpoint=BREAKPOINT>
                            <sized @sizing=ItemSize::Percent(50.0) @node_ref=&left height=ITEM_HEIGHT>
                                <spacer />
                            </sized>
                            <sized @sizing=ItemSize::Percent(50.0) @node_ref=&right height=ITEM_HEIGHT>
                                <spacer />
                            </sized>
                        </stack>
                    }}
                </container>
            }
        }
    });
    let (left, right) = (left.get(), right.get());

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), WIDE_VIEWPORT.x / 2.0);
    assert_eq!(harness.rect(right).left(), WIDE_VIEWPORT.x / 2.0);
    assert_eq!(harness.rect(right).top(), harness.rect(left).top());

    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), VIEWPORT.x);
    assert_eq!(harness.rect(right).width(), VIEWPORT.x);
    assert_eq!(harness.rect(right).top(), harness.rect(left).bottom());
}
