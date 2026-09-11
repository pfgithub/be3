use super::*;
use crate::reactive::{build, create_memo, create_signal, view, NodeRef};
use crate::styled::SwitchBuilder;

#[test]
fn flipping_a_switch_can_replace_the_items_of_a_scroll() {
    for inset in [7.0, 17.0] {
        check_compact_rows(inset);
    }
}

fn check_compact_rows(inset: f32) {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (switch, scroll) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (switch, scroll, sink) = (switch.clone(), scroll.clone(), built.clone());
        move || {
            let (compact, set_compact) = create_signal(false);
            let item_height = create_memo(move || {
                if compact.get() {
                    VIRTUAL_ITEM_HEIGHT / 2.0
                } else {
                    VIRTUAL_ITEM_HEIGHT
                }
            });
            let row_height = item_height.clone();
            view! {
                <column spacing={0.0}>
                    <switch
                        node_ref={&switch}
                        on={false}
                        on_change={move |on: bool| set_compact.set(on)}
                    />
                    @percent(100.0) <virtual_list
                        node_ref={&scroll}
                        count={VIRTUAL_ITEM_COUNT}
                        item_height={item_height}
                        item={move |index: usize| {
                            sink.borrow_mut().push(index);
                            let height = row_height.get() / 2.0;
                            view! {
                                <padding horizontal={0.0} vertical={height}><spacer /></padding>
                            }
                        }}
                    />
                </column>
            }
        }
    });
    let (switch, scroll) = (switch.get(), scroll.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let index = 120;
    harness
        .document
        .set_scroll_offset(scroll, index as f32 * VIRTUAL_ITEM_HEIGHT + inset);
    harness.frame(Vec::new());
    let top = harness.rect(harness.document.children(scroll)[0]).top();
    built.borrow_mut().clear();

    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert!(styled::switch_on(harness.document(), switch));
    let skipped = (inset / (VIRTUAL_ITEM_HEIGHT / 2.0)) as usize;
    assert_eq!(built.borrow().first(), Some(&(index + skipped)));
    assert_eq!(
        harness.rect(harness.document.children(scroll)[0]).top(),
        top + skipped as f32 * VIRTUAL_ITEM_HEIGHT / 2.0
    );
    assert_eq!(
        harness.document.scroll_offset(scroll),
        index as f32 * VIRTUAL_ITEM_HEIGHT / 2.0 + inset
    );

    built.borrow_mut().clear();
    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert_eq!(built.borrow().first(), Some(&index));
    assert_eq!(
        harness.rect(harness.document.children(scroll)[0]).top(),
        top
    );
    assert_eq!(
        harness.document.scroll_offset(scroll),
        index as f32 * VIRTUAL_ITEM_HEIGHT + inset
    );
}
