use super::*;
use crate::input::CursorIcon;
use crate::reactive::with_document;

fn slotted_shadow(document: &mut Document) -> NodeId {
    let slot = document.create_slot("content");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, slot);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);
    let shadow = document.create_shadow("button", focusable, vec![slot]);
    let fill = document.create_fill(Color32::from_gray(60), 4);
    document.set_slot_child(slot, fill);
    shadow
}

#[test]
fn the_inspector_separates_component_internals_from_slots() {
    let (document, [_button]) = toolbar_of(|| [with_document(slotted_shadow)]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "    content",
            "      fill"
        ]
    );

    let shadow_row = harness.marker_center(2);
    harness.click(shadow_row);
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "      focusable",
            "    content",
            "      fill",
        ]
    );

    harness.click(harness.marker_center(3));
    harness.frame(Vec::new());
    harness.click(harness.marker_center(4));
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  button",
            "    shadow",
            "      focusable",
            "        click-catcher",
            "          slot",
            "    content",
            "      fill",
        ]
    );
}
