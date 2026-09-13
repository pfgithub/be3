use super::*;
use crate::reactive::{build, view};
use crate::styled::ResponsiveTabs;
use crate::unstyled::Container;

const BREAKPOINT: f32 = 500.0;

#[test]
fn tabs_collapse_into_a_select_when_their_container_is_narrow() {
    let tabs = NodeRef::new();
    let document = build({
        let tabs = tabs.clone();
        move || {
            view! {
                <Container>
                    {move |_| view! {
                        <ResponsiveTabs
                            @node_ref=&tabs
                            labels={vec!["List".to_string(), "Load".to_string(), "Name".to_string()]}
                            selected=1
                            breakpoint=BREAKPOINT
                        />
                    }}
                </Container>
            }
        }
    });
    let tabs = tabs.get();

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let column = tabs;
    let [wide, narrow] = harness.document().children(column)[..] else {
        panic!("responsive tabs hold a branch for each width");
    };
    let (wide, narrow) = (wide, narrow);
    assert!(harness.document().is_visible(wide));
    assert!(!harness.document().is_visible(narrow));
    assert!(harness.document().children(narrow).is_empty());
    assert_eq!(
        styled::responsive_tabs_selected(harness.document(), tabs),
        1
    );

    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    assert!(!harness.document().is_visible(wide));
    assert!(harness.document().is_visible(narrow));
    let select = harness.document().children(narrow)[0];
    assert!(harness.document().node_rect(select).is_some());
    assert!(harness
        .document()
        .node_rect(harness.document().children(wide)[0])
        .is_none());
    assert_eq!(styled::select_selected(harness.document(), select), Some(1));
}
