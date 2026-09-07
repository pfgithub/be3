use super::*;

#[test]
fn caret_repaints_on_a_deadline_without_repeating_layout() {
    let mut document = Document::new();
    let text = document.create_text("hello", 14.0, Color32::WHITE);
    document.set_text_caret(text, Some(0));
    document.set_root(text);
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
