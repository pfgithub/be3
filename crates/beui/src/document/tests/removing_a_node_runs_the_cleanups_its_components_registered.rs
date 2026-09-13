use super::*;
use crate::reactive::{component, on_cleanup, view, Text};

#[test]
fn removing_a_node_runs_the_cleanups_its_components_registered() {
    let cleaned: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let outer_sink = cleaned.clone();
    let inner_sink = cleaned.clone();
    let (document, [outer]) = toolbar_of(move || {
        [component(move || {
            on_cleanup(move || outer_sink.borrow_mut().push("outer"));
            component(move || {
                on_cleanup(move || inner_sink.borrow_mut().push("inner"));
                view! { <Text string="hi" /> }
            })
        })]
    });
    let list = document.root().expect("the toolbar is the root");

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(cleaned.borrow().is_empty());

    harness.document_mut().remove_child(list, outer);
    harness.document_mut().remove_node(outer);
    harness.frame(Vec::new());

    assert_eq!(*cleaned.borrow(), ["inner", "outer"]);
}
