use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn hovering_a_select_option_moves_the_keyboard_highlight() {
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let (document, [select]) = toolbar_of(|| [view! { <Select options selected=None /> }]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = select;
    let trigger = unstyled::select_trigger(harness.document(), inner);
    with_installed(harness.document_mut(), |_| {
        crate::focus_within(trigger);
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
