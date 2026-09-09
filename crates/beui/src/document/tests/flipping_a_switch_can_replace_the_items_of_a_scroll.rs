use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SwitchBuilder;

#[test]
fn flipping_a_switch_can_replace_the_items_of_a_scroll() {
    for inset in [7.0, 17.0] {
        check_compact_rows(inset);
    }
}

fn check_compact_rows(inset: f32) {
    let built = Rc::new(RefCell::new(Vec::new()));
    let mut document = Document::new();
    let scroll = document.create_scroll();

    let rebuilt = built.clone();
    let switch = with_reactive_scope(&mut document, || {
        view! {
            <switch on={false} on_change={Box::new(move |document: &mut Document, on| {
                let height = if on {
                    VIRTUAL_ITEM_HEIGHT / 2.0
                } else {
                    VIRTUAL_ITEM_HEIGHT
                };
                install_scroll_items(document, scroll, height, &rebuilt);
            })} />
        }
    });
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, switch, ItemSize::Intrinsic);
    document.append_child(column, scroll, ItemSize::Percent(100.0));
    document.set_root(column);

    let first = built.clone();
    install_scroll_items(&mut document, scroll, VIRTUAL_ITEM_HEIGHT, &first);

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

fn install_scroll_items(
    document: &mut Document,
    scroll: NodeId,
    height: f32,
    built: &Rc<RefCell<Vec<usize>>>,
) {
    let sink = built.clone();
    document.set_scroll_virtual_items(
        scroll,
        VIRTUAL_ITEM_COUNT,
        height,
        move |document, index| {
            sink.borrow_mut().push(index);
            document.create_padding(0.0, height / 2.0)
        },
    );
}
