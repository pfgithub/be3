use super::*;
use crate::reactive::{build, button, column, create_signal, intrinsic, text};

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

        let toggle = button()
            .children([intrinsic(text().string("toggle".to_string()).build())])
            .on_click(Box::new(move |_document| {
                set_disabled.update(|disabled| *disabled = !*disabled)
            }))
            .build();
        sink_toggle.set(Some(toggle));

        let go = button()
            .children([intrinsic(text().string("go".to_string()).build())])
            .disabled(disabled)
            .on_click(Box::new(move |_document| sink.set(sink.get() + 1)))
            .build();
        sink_go.set(Some(go));

        column()
            .spacing(0.0)
            .children([intrinsic(toggle), intrinsic(go)])
            .build()
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
