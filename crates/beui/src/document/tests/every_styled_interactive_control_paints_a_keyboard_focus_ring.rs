use super::*;
use crate::reactive::{view, Text};
use crate::styled::{
    Accordion, Button, Checkbox, ListRow, Listbox, RadioGroup, Slider, Switch, Tabs, TextInput,
    ToggleButton,
};

#[test]
fn every_styled_interactive_control_paints_a_keyboard_focus_ring() {
    let controls: &[fn() -> NodeId] = &[
        || {
            view! {
                <Button label="Button" variant=styled::ButtonVariant::Primary />
            }
        },
        || view! { <Checkbox label="Check" checked=false /> },
        || view! { <Switch on=false /> },
        || view! { <Slider value=0.5 /> },
        || view! { <TextInput value="Text" /> },
        || view! { <Tabs labels={vec!["One".to_string(), "Two".to_string()]} selected=0 /> },
        || {
            view! {
                <RadioGroup labels={vec!["One".to_string(), "Two".to_string()]} selected=Some(0) />
            }
        },
        || {
            view! {
                <Listbox labels={vec!["One".to_string(), "Two".to_string()]} selected=Some(0) />
            }
        },
        || view! { <ToggleButton label="Toggle" pressed=false /> },
        || {
            view! {
                <Accordion title="Header" open=false>
                    <Text string="Content" font_size=14.0 color=Color32::WHITE />
                </Accordion>
            }
        },
        || {
            view! {
                <ListRow>
                    <Text string="Row" font_size=14.0 color=Color32::WHITE />
                </ListRow>
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
