use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::RadioGroupBuilder;

#[test]
fn clicking_a_choice_keeps_keyboard_focus_on_the_selected_option() {
    let mut document = Document::new();
    let group = with_reactive_scope(&mut document, || {
        view! { <radio_group labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string()]} selected={Some(0)} /> }
    });
    toolbar(&mut document, &[group]);
    let options = document.children(document.shadow_root(document.shadow_root(group)));
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.click(harness.center(options[1]));
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        Some(1)
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        Some(2)
    );
}
