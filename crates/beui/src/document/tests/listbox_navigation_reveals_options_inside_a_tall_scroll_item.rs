use super::*;
use crate::reactive::{build, view, NodeRef, ScrollBuilder};
use crate::styled::ListboxBuilder;

#[test]
fn listbox_navigation_reveals_options_inside_a_tall_scroll_item() {
    let (scroll, listbox) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, listbox) = (scroll.clone(), listbox.clone());
        move || {
            view! {
                <scroll node_ref=&scroll>
                    <listbox
                        node_ref=&listbox
                        labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string(), "Four".to_string(), "Five".to_string(), "Six".to_string()]}
                        selected=Some(0)
                    />
                </scroll>
            }
        }
    });
    let (scroll, listbox) = (scroll.get(), listbox.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 80.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(5)
    );
    let focused = harness.document.focused_node().unwrap();
    let rect = harness.rect(focused);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 80.0);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
