use super::*;
use crate::reactive::{view, with_document, with_reactive_scope};
use crate::styled::{
    AccordionBuilder, ButtonBuilder, CheckboxBuilder, ListRowBuilder, SliderBuilder, SwitchBuilder,
    ToggleButtonBuilder,
};

#[test]
fn every_styled_interactive_control_paints_a_keyboard_focus_ring() {
    let builders: &[fn(&mut Document) -> NodeId] = &[
        |doc| {
            with_reactive_scope(doc, || {
                view! { <button label={"Button".to_string()} variant={styled::ButtonVariant::Primary} /> }
            })
        },
        |doc| {
            with_reactive_scope(doc, || {
                view! { <checkbox label={"Check".to_string()} checked={false} /> }
            })
        },
        |doc| with_reactive_scope(doc, || view! { <switch on={false} /> }),
        |doc| with_reactive_scope(doc, || view! { <slider value={0.5} /> }),
        |doc| styled::text_input(doc, "Text"),
        |doc| styled::tabs(doc, &["One", "Two"], 0),
        |doc| styled::radio_group(doc, &["One", "Two"], Some(0)),
        |doc| styled::listbox(doc, &["One", "Two"], Some(0)),
        |doc| {
            with_reactive_scope(doc, || {
                view! { <toggle_button label={"Toggle".to_string()} pressed={false} /> }
            })
        },
        |doc| {
            with_reactive_scope(doc, || {
                let text =
                    with_document(|document| document.create_text("Content", 14.0, Color32::WHITE));
                view! { <accordion title={"Header".to_string()} open={false}>{text}</accordion> }
            })
        },
        |doc| {
            with_reactive_scope(doc, || {
                let text =
                    with_document(|document| document.create_text("Row", 14.0, Color32::WHITE));
                view! { <list_row>{text}</list_row> }
            })
        },
    ];
    for build in builders {
        let mut document = Document::new();
        let control = build(&mut document);
        toolbar(&mut document, &[control]);
        let mut harness = Harness::new(document);
        let initial = harness.frame(vec![]);
        let focused = harness.frame(vec![key_event(Key::Tab, true, Modifiers::NONE)]);
        let rings = |output: &crate::FrameOutput| {
            output.shapes().iter().filter(|shape| matches!(shape,
            crate::Shape::Rect { color, stroke_width, .. } if *color == styled::theme::ACCENT && *stroke_width == 2.0
        )).count()
        };
        assert!(
            rings(&focused) > rings(&initial),
            "{} has no focus ring",
            harness.document.node_kind(control)
        );
    }
}
