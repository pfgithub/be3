use super::*;
use crate::reactive::{build, create_signal, view, Button, Column, Text};

#[test]
fn a_disabled_button_prop_tracks_a_signal_and_blocks_clicks_while_true() {
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
    let sink = clicks.clone();
    let go_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_go = go_id.clone();
    let toggle_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_toggle = toggle_id.clone();

    let document = build(move || {
        let (disabled, set_disabled) = create_signal(true);

        let toggle = view! {
            <Button on_click={move || {
                set_disabled.update(|disabled| *disabled = !*disabled)
            }}>
                <Text string="toggle" />
            </Button>
        };
        sink_toggle.set(Some(toggle));

        let go = view! {
            <Button disabled on_click={move || sink.set(sink.get() + 1)}>
                <Text string="go" />
            </Button>
        };
        sink_go.set(Some(go));

        view! {
            <Column spacing=0.0>
                {toggle}
                {go}
            </Column>
        }
    });

    let toggle = toggle_id.get().expect("toggle button was created");
    let go = go_id.get().expect("go button was created");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(go));
    harness.frame(Vec::new());
    assert_eq!(clicks.get(), 0, "a disabled button must not fire on_click");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    harness.click(harness.center(go));
    harness.frame(Vec::new());
    assert_eq!(
        clicks.get(),
        1,
        "the button must become clickable once its disabled prop turns false"
    );
}
