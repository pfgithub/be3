use super::*;
use crate::reactive::{build, view, ItemSize, Memo, ReadSignal};
use crate::styled::Stack;
use crate::unstyled::{narrower_than, Container};

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
                <Container>
                    {move |size| {
                        sizes.borrow_mut().push(size);
                        collapsed.borrow_mut().push(narrower_than(BREAKPOINT));
                        view! {
                            <Column spacing=0.0>
                                <Stack spacing=0.0 breakpoint=BREAKPOINT>
                                    <Sized @sizing=ItemSize::Percent(100.0) @node_ref=&outer_item height=ITEM_HEIGHT>
                                        <Spacer />
                                    </Sized>
                                </Stack>
                                <Sized width=INNER_WIDTH>
                                    <Container>
                                        {move |size| {
                                            inner_sizes.borrow_mut().push(size);
                                            inner_collapsed
                                                .borrow_mut()
                                                .push(narrower_than(BREAKPOINT));
                                            view! {
                                                <Stack spacing=0.0 breakpoint=BREAKPOINT>
                                                    <Sized @sizing=ItemSize::Percent(100.0)
                                                        @node_ref=&inner_item
                                                        height=ITEM_HEIGHT
                                                    >
                                                        <Spacer />
                                                    </Sized>
                                                </Stack>
                                            }
                                        }}
                                    </Container>
                                </Sized>
                            </Column>
                        }
                    }}
                </Container>
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
