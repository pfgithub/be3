use super::*;
use crate::reactive::{view, Text};
use crate::styled::ListRow;

#[test]
fn the_inspector_shows_the_base_nodes_of_a_styled_component() {
    let (document, [_row]) = toolbar_of(|| {
        [view! {
            <ListRow>
                <Text string="Hello" font_size=14.0 color=Color32::WHITE />
            </ListRow>
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  focusable",
            "    click-catcher",
            "      outline"
        ]
    );
}
