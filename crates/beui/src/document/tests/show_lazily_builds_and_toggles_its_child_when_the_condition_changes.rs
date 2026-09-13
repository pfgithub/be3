use super::*;
use crate::reactive::{build, create_signal, view, Button, Column, NodeRef, Show, Text};

#[test]
fn show_lazily_builds_and_toggles_its_child_when_the_condition_changes() {
    let builds = std::rc::Rc::new(std::cell::Cell::new(0));
    let sink = builds.clone();
    let (toggle, panel) = (NodeRef::new(), NodeRef::new());

    let document = build({
        let (toggle, panel) = (toggle.clone(), panel.clone());
        move || {
            let (visible, set_visible) = create_signal(false);
            view! {
                <Column spacing=0.0>
                    <Button
                        @node_ref=&toggle
                        on_click={move || set_visible.update(|visible| *visible = !*visible)}
                    >
                        <Text string="toggle" />
                    </Button>
                    <Show
                        @node_ref=&panel
                        condition={visible}
                        then={move || {
                            sink.set(sink.get() + 1);
                            view! { <Text string="panel" /> }
                        }}
                    />
                </Column>
            }
        }
    });

    let (toggle, panel) = (toggle.get(), panel.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let visibility = panel;

    assert_eq!(builds.get(), 0, "a hidden show() must not build its child");
    assert!(!harness.document().is_visible(visibility));

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get(),
        1,
        "showing it for the first time must build it"
    );
    assert!(harness.document().is_visible(visibility));
    let child = harness.document().children(visibility)[0];
    assert_eq!(text_of(harness.document(), child), "panel");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert!(!harness.document().is_visible(visibility));
    assert_eq!(builds.get(), 1, "hiding it again must not rebuild it");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert!(harness.document().is_visible(visibility));
    assert_eq!(
        builds.get(),
        1,
        "showing it a second time must reuse the already-built child"
    );
}
