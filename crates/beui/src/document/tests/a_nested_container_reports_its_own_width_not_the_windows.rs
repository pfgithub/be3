use super::*;
use crate::reactive::{build, view, Memo, ReadSignal};
use crate::styled::StackBuilder;
use crate::unstyled::{narrower_than, ContainerBuilder};

const INNER_WIDTH: f32 = 200.0;
const BREAKPOINT: f32 = 300.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_nested_container_reports_its_own_width_not_the_windows() {
    let sizes: Rc<RefCell<Vec<ReadSignal<Vec2>>>> = Rc::default();
    let collapsed: Rc<RefCell<Vec<Memo<bool>>>> = Rc::default();
    let (outer_item, inner_item) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (sizes, collapsed) = (sizes.clone(), collapsed.clone());
        let (outer_item, inner_item) = (outer_item.clone(), inner_item.clone());
        move || {
            let inner_sizes = sizes.clone();
            let inner_collapsed = collapsed.clone();
            view! {
                <container content={move |size| {
                    sizes.borrow_mut().push(size);
                    collapsed.borrow_mut().push(narrower_than(BREAKPOINT));
                    view! {
                        <column spacing={0.0}>
                            <stack spacing={0.0} breakpoint={BREAKPOINT}>
                                @percent(100.0) <sized node_ref={&outer_item} height={ITEM_HEIGHT}>
                                    <spacer />
                                </sized>
                            </stack>
                            <sized width={INNER_WIDTH}>
                                <container content={move |size| {
                                    inner_sizes.borrow_mut().push(size);
                                    inner_collapsed.borrow_mut().push(narrower_than(BREAKPOINT));
                                    view! {
                                        <stack spacing={0.0} breakpoint={BREAKPOINT}>
                                            @percent(100.0) <sized node_ref={&inner_item} height={ITEM_HEIGHT}>
                                                <spacer />
                                            </sized>
                                        </stack>
                                    }
                                }} />
                            </sized>
                        </column>
                    }
                }} />
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let sizes = sizes.borrow();
    assert_eq!(sizes[0].get().x, WIDE_VIEWPORT.x);
    assert_eq!(sizes[1].get().x, INNER_WIDTH);

    let collapsed = collapsed.borrow();
    assert!(!collapsed[0].get());
    assert!(collapsed[1].get());

    assert_eq!(harness.rect(outer_item.get()).width(), WIDE_VIEWPORT.x);
    assert_eq!(harness.rect(inner_item.get()).width(), INNER_WIDTH);
}
