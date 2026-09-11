use super::*;
use crate::reactive::{view, TextBuilder};
use crate::styled::ListRowBuilder;

#[test]
fn the_inspector_hides_the_internals_of_a_styled_component() {
    let (document, [_row]) = toolbar_of(|| {
        [view! {
            <list_row>
                <text string={"Hello".to_string()} font_size={14.0} color={Color32::WHITE} />
            </list_row>
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        ["column", "  list_row", "    button", "      focusable"]
    );
}
