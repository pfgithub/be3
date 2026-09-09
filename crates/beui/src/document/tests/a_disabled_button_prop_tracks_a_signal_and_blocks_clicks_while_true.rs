use super::*;
use crate::reactive::{build, create_signal, view, ButtonBuilder, ColumnBuilder, TextBuilder};

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
            <button on_click={Box::new(move |_document| {
                set_disabled.update(|disabled| *disabled = !*disabled)
            })}>
                <text string={"toggle".to_string()} />
            </button>
        };
        sink_toggle.set(Some(toggle));

        let go = view! {
            <button disabled={disabled} on_click={Box::new(move |_document| sink.set(sink.get() + 1))}>
                <text string={"go".to_string()} />
            </button>
        };
        sink_go.set(Some(go));

        view! {
            <column spacing={0.0}>
                {toggle}
                {go}
            </column>
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
