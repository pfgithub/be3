use super::*;
use crate::reactive::{build, intrinsic, view, NodeRef, ScrollBuilder, TextBuilder};
use crate::styled::{SliderBuilder, TabsBuilder};

#[test]
fn unused_navigation_keys_scroll_the_nearest_ancestor() {
    let (scroll, tabs, slider) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, tabs, slider) = (scroll.clone(), tabs.clone(), slider.clone());
        move || {
            let mut items = vec![
                intrinsic(view! {
                    <tabs
                        node_ref=&tabs
                        labels={vec!["One".to_string(), "Two".to_string()]}
                        selected=0
                    />
                }),
                intrinsic(view! { <slider node_ref=&slider value=0.5 /> }),
            ];
            items.extend((0..20).map(|_| {
                intrinsic(view! {
                    <text string="Content" font_size=14.0 color=Color32::WHITE />
                })
            }));
            view! { <scroll node_ref=&scroll children={items} /> }
        }
    });
    let (scroll, tabs, slider) = (scroll.get(), tabs.get(), slider.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);
    assert_eq!(harness.document.scroll_offset(scroll), 40.0);
    harness.key(Key::PageUp, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert!(styled::slider_value(harness.document(), slider) < 0.5);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
