use super::*;

#[test]
fn copy_and_cut_export_only_selected_text_and_cut_can_be_undone() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "Hello world");
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    let output = harness.frame(vec![key_event(Key::C, true, Modifiers::CTRL)]);
    assert_eq!(output.copied_text, None);
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(
        Key::ArrowRight,
        Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    );
    let output = harness.frame(vec![key_event(Key::C, true, Modifiers::CTRL)]);
    assert_eq!(output.copied_text.as_deref().map(str::trim), Some("Hello"));
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Hello world"
    );
    harness.key(Key::A, Modifiers::CTRL);
    let output = harness.frame(vec![key_event(Key::X, true, Modifiers::CTRL)]);
    assert_eq!(output.copied_text.as_deref(), Some("Hello world"));
    assert_eq!(styled::text_input_value(harness.document(), input), "");
    harness.key(Key::Z, Modifiers::CTRL);
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Hello world"
    );
    harness.key(Key::A, Modifiers::CTRL);
    harness.frame(vec![Event::Text("Pasted text".to_owned())]);
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Pasted text"
    );
}
