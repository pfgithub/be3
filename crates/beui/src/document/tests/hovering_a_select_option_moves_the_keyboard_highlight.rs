use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SelectBuilder;

#[test]
fn hovering_a_select_option_moves_the_keyboard_highlight() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = with_reactive_scope(&mut document, || {
        view! { <select options={options} selected={None} /> }
    });
    toolbar(&mut document, &[select]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    with_installed(harness.document_mut(), |_document| {
        unstyled::focus_button(trigger);
    });
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );

    let cherry = unstyled::select_option_button(harness.document(), inner, 2);
    let cherry_pos = harness.center(cherry);
    harness.frame(vec![Event::PointerMoved(cherry_pos)]);

    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(2)
    );

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(1)
    );
}
