use super::*;
use crate::reactive::{
    build, create_signal, view, NodeRef, TextBuilder, VirtualListBuilder, VisibilityBuilder,
};

#[test]
fn evicting_a_virtual_scroll_row_disposes_its_effects() {
    let (shown, set_shown) = create_signal(true);
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <column spacing=0.0>
                    @percent(100.0) <virtual_list
                        node_ref=&scroll
                        count=100
                        item_height=20.0
                    >
                        {move |index: usize| {
                            let shown = shown.clone();
                            view! {
                                <visibility visible={shown}>
                                    <text string={format!("Row {index}")} />
                                </visibility>
                            }
                        }}
                    </virtual_list>
                </column>
            }
        }
    });
    let scroll = scroll.get();

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    *harness.viewport_mut() = Vec2::new(400.0, 0.0);
    harness.frame(Vec::new());
    assert!(harness.document().children(scroll).is_empty());
    *harness.viewport_mut() = Vec2::new(400.0, 300.0);
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    with_installed(harness.document_mut(), |_| set_shown.set(false));
    harness.frame(Vec::new());
}
