use super::*;
use crate::reactive::{view, TextBuilder};
use crate::styled::{
    AccordionBuilder, ButtonBuilder, CheckboxBuilder, ListRowBuilder, ListboxBuilder,
    RadioGroupBuilder, SliderBuilder, SwitchBuilder, TabsBuilder, TextInputBuilder,
    ToggleButtonBuilder,
};

#[test]
fn every_styled_interactive_control_paints_a_keyboard_focus_ring() {
    let controls: &[fn() -> NodeId] = &[
        || {
            view! {
                <button label={"Button".to_string()} variant={styled::ButtonVariant::Primary} />
            }
        },
        || view! { <checkbox label={"Check".to_string()} checked={false} /> },
        || view! { <switch on={false} /> },
        || view! { <slider value={0.5} /> },
        || view! { <text_input value={"Text".to_string()} /> },
        || view! { <tabs labels={vec!["One".to_string(), "Two".to_string()]} selected={0} /> },
        || {
            view! {
                <radio_group labels={vec!["One".to_string(), "Two".to_string()]} selected={Some(0)} />
            }
        },
        || {
            view! {
                <listbox labels={vec!["One".to_string(), "Two".to_string()]} selected={Some(0)} />
            }
        },
        || view! { <toggle_button label={"Toggle".to_string()} pressed={false} /> },
        || {
            view! {
                <accordion title={"Header".to_string()} open={false}>
                    <text string={"Content".to_string()} font_size={14.0} color={Color32::WHITE} />
                </accordion>
            }
        },
        || {
            view! {
                <list_row>
                    <text string={"Row".to_string()} font_size={14.0} color={Color32::WHITE} />
                </list_row>
            }
        },
    ];
    for control in controls {
        let (document, [control]) = toolbar_of(|| [control()]);
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
