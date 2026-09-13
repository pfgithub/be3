use super::*;
use crate::input::CursorIcon;
use crate::reactive::{component, view, Child, ClickCatcher, Focusable};

#[component]
fn Slotted(children: Child) -> NodeId {
    view! {
        <Focusable>
            <ClickCatcher cursor=CursorIcon::PointingHand>
                {children}
            </ClickCatcher>
        </Focusable>
    }
}

#[test]
fn the_inspector_separates_component_internals_from_slots() {
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <Slotted>
                <Fill color=Color32::from_gray(60) radius=4 />
            </Slotted>
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  Slotted",
            "    shadow",
            "    children",
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
            "  Slotted",
            "    shadow",
            "      focusable",
            "    children",
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
            "  Slotted",
            "    shadow",
            "      focusable",
            "        click-catcher",
            "          slot",
            "    children",
            "      fill",
        ]
    );
}
