use super::*;
use crate::reactive::create_memo;
use crate::styled::Stack;
use crate::unstyled::{narrower_than, Container};

const BREAKPOINT: f32 = 500.0;
const STACKED_LIST_HEIGHT: f32 = 100.0;

#[test]
fn a_virtual_list_in_a_stacked_stack_only_builds_the_items_in_view() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let sink = built.clone();
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <Container>
                    {move |_| {
                    let (sink, scroll) = (sink.clone(), scroll.clone());
                    let narrow = narrower_than(BREAKPOINT);
                    let size = create_memo(move || if narrow.get() {
                        ItemSize::Fixed(STACKED_LIST_HEIGHT)
                    } else {
                        ItemSize::Percent(100.0)
                    });
                    view! {
                        <Stack spacing=0.0 breakpoint=BREAKPOINT>
                            <Spacer @sizing=ItemSize::Percent(50.0) />
                            <Column @sizing=ItemSize::Percent(50.0) spacing=0.0>
                                <Column @sizing={size} spacing=0.0>
                                    <VirtualList @sizing=ItemSize::Percent(100.0)
                                        @node_ref=&scroll
                                        count=VIRTUAL_ITEM_COUNT
                                        item_height=VIRTUAL_ITEM_HEIGHT
                                    >
                                        {move |index: usize| {
                                            sink.borrow_mut().push(index);
                                            view! {
                                                <Padding horizontal=0.0 vertical={VIRTUAL_ITEM_HEIGHT / 2.0}>
                                                    <Spacer />
                                                </Padding>
                                            }
                                        }}
                                    </VirtualList>
                                </Column>
                            </Column>
                        </Stack>
                    }
                    }}
                </Container>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let wide = (WIDE_VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(harness.document.children(scroll.get()).len(), wide);

    built.borrow_mut().clear();
    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    let stacked = (STACKED_LIST_HEIGHT / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(harness.rect(scroll.get()).height(), STACKED_LIST_HEIGHT);
    assert_eq!(harness.document.children(scroll.get()).len(), stacked);
    assert_eq!(*built.borrow(), Vec::new());
}
