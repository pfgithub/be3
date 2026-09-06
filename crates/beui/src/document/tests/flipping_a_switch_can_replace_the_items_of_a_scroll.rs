use super::*;

#[test]
fn flipping_a_switch_can_replace_the_items_of_a_scroll() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let switch = styled::switch(&mut document, false);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, switch, ItemSize::Intrinsic);
    document.append_child(column, scroll, ItemSize::Percent(100.0));
    document.set_root(column);

    let first = built.clone();
    install_scroll_items(&mut document, scroll, VIRTUAL_ITEM_HEIGHT, &first);
    let rebuilt = built.clone();
    styled::set_switch_on_change(&mut document, switch, move |document, on| {
        let height = if on {
            VIRTUAL_ITEM_HEIGHT / 2.0
        } else {
            VIRTUAL_ITEM_HEIGHT
        };
        install_scroll_items(document, scroll, height, &rebuilt);
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let before = built.borrow().len();

    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert!(styled::switch_on(harness.document(), switch));
    assert!(
        built.borrow().len() > before,
        "the scroll never rebuilt its items"
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
