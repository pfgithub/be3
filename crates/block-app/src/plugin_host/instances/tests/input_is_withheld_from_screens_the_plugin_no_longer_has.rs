use super::*;

#[test]
fn input_is_withheld_from_screens_the_plugin_no_longer_has() {
    let (mut instances, context, id) = placed();
    let screens = instances.next_screens(PASS).screens;
    assert_eq!(screens.len(), 1);
    instances.screen_set(screens);
    context.memory_mut(|memory| memory.request_focus(id));

    let messages = instances.frame_input(&context, PASS);

    assert!(matches!(
        messages.as_slice(),
        [Message::Input(batch)] if batch.events == [InputEvent::Focus(true)]
    ));

    instances.screen_set(Vec::new());
    context.memory_mut(|memory| memory.surrender_focus(id));

    assert!(instances.frame_input(&context, PASS).is_empty());
}
