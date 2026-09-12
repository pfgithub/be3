use super::*;
use crate::reactive::{build, create_signal, view, Button, Column, NodeRef, Show, Text};

type Builds = Rc<Cell<usize>>;

#[component]
fn CountedPanel(builds: Builds) -> NodeId {
    builds.set(builds.get() + 1);
    view! { <Text string="panel" /> }
}

#[test]
fn children_written_between_show_tags_are_not_built_until_it_is_shown() {
    let builds: Builds = Rc::new(Cell::new(0));
    let (toggle, panel) = (NodeRef::new(), NodeRef::new());

    let document = build({
        let (builds, toggle, panel) = (builds.clone(), toggle.clone(), panel.clone());
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
                    <Show @node_ref=&panel condition=visible>
                        <CountedPanel builds />
                    </Show>
                </Column>
            }
        }
    });

    let (toggle, panel) = (toggle.get(), panel.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let visibility = harness.document().shadow_root(panel);

    assert_eq!(
        builds.get(),
        0,
        "a children block filling a render prop must stay unbuilt while it is hidden"
    );
    assert!(!harness.document().is_visible(visibility));

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(builds.get(), 1, "showing it must build the children block");
    assert!(harness.document().is_visible(visibility));
    let child = harness.document().children(visibility)[0];
    assert_eq!(
        text_of(harness.document(), harness.document().shadow_root(child)),
        "panel"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert!(harness.document().is_visible(visibility));
    assert_eq!(
        builds.get(),
        1,
        "hiding and showing it again must reuse the child it already built"
    );
}
