use super::*;
use crate::reactive::{build, create_signal, view, ButtonBuilder, ShowBuilder};
use crate::styled::StackBuilder;
use crate::unstyled::ContainerBuilder;

const BREAKPOINT: f32 = 500.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_stack_built_inside_a_show_still_measures_the_container_above_it() {
    let (toggle, left, right) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (toggle, left, right) = (toggle.clone(), left.clone(), right.clone());
        move || {
            let (visible, set_visible) = create_signal(false);
            view! {
                <container content={move |_| view! {
                    <column spacing={0.0}>
                        <button node_ref={&toggle} on_click={move || set_visible.set(true)}>
                            <text string={"toggle".to_string()} />
                        </button>
                        <show condition={visible} then={move || view! {
                            <stack spacing={0.0} breakpoint={BREAKPOINT}>
                                @percent(50.0) <sized node_ref={&left} height={ITEM_HEIGHT}>
                                    <spacer />
                                </sized>
                                @percent(50.0) <sized node_ref={&right} height={ITEM_HEIGHT}>
                                    <spacer />
                                </sized>
                            </stack>
                        }} />
                    </column>
                }} />
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(toggle.get()));
    harness.frame(Vec::new());

    let (left, right) = (left.get(), right.get());
    assert_eq!(harness.rect(left).width(), VIEWPORT.x);
    assert_eq!(harness.rect(right).top(), harness.rect(left).bottom());
}
