use super::*;
use crate::reactive::{build, view, NodeRef, TextBuilder};

#[test]
fn caret_repaints_on_a_deadline_without_repeating_layout() {
    let text = NodeRef::new();
    let mut document = build({
        let text = text.clone();
        move || {
            view! {
                <text
                    node_ref={&text}
                    string={"hello".to_string()}
                    font_size={14.0}
                    color={Color32::WHITE}
                    caret={Some(0)}
                />
            }
        }
    });
    let text = text.get();
    let (layouts, paints) = counted(&mut document, text);
    let mut harness = Harness::new(document);
    let output = harness.frame(vec![]);
    assert!(!output.repaint);
    assert!(output.repaint_after > Duration::ZERO);
    assert!(output.repaint_after <= Duration::from_millis(530));
    assert!(!harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    harness.document.next_paint = Some(Instant::now());
    harness.frame(vec![]);
    assert_eq!((layouts.get(), paints.get()), (1, 2));
    harness.document.set_text_caret(text, None);
    assert_eq!(harness.frame(vec![]).repaint_after, Duration::MAX);
}
