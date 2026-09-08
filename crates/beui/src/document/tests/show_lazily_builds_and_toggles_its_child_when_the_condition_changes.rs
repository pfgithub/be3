use super::*;
use crate::reactive::{build, button, column, create_signal, intrinsic, show, text};

#[test]
fn show_lazily_builds_and_toggles_its_child_when_the_condition_changes() {
    let builds = std::rc::Rc::new(std::cell::Cell::new(0));
    let sink = builds.clone();
    let toggle_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_toggle = toggle_id.clone();
    let panel_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_panel = panel_id.clone();

    let document = build(move || {
        let (visible, set_visible) = create_signal(false);

        let toggle = button()
            .label(text("toggle"))
            .on_click(move || set_visible.update(|visible| *visible = !*visible))
            .build();
        sink_toggle.set(Some(toggle));

        let panel = show(visible, move || {
            sink.set(sink.get() + 1);
            text("panel")
        });
        sink_panel.set(Some(panel));

        column(0.0, [intrinsic(toggle), intrinsic(panel)])
    });

    let toggle = toggle_id.get().expect("toggle button was created");
    let panel = panel_id.get().expect("show() node was created");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(builds.get(), 0, "a hidden show() must not build its child");
    assert!(!harness.document().is_visible(panel));

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get(),
        1,
        "showing it for the first time must build it"
    );
    assert!(harness.document().is_visible(panel));
    let child = harness.document().children(panel)[0];
    assert_eq!(harness.document().text(child), "panel");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert!(!harness.document().is_visible(panel));
    assert_eq!(builds.get(), 1, "hiding it again must not rebuild it");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert!(harness.document().is_visible(panel));
    assert_eq!(
        builds.get(),
        1,
        "showing it a second time must reuse the already-built child"
    );
}
