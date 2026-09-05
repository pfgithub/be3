use super::*;

#[test]
fn a_message_waits_for_the_instance_it_names_to_be_opened() {
    let (mut instances, ..) = placed();
    let presence = Message::Editor(EditorMessage::Presence {
        instance: INSTANCE,
        visible: true,
        entries: Vec::new(),
    });

    assert!(instances.gate(vec![presence.clone()]).is_empty());

    let opened = instances.next_screens(PASS).opened;
    let open = opened
        .iter()
        .position(|message| matches!(message, Message::Editor(EditorMessage::Open { .. })));
    let held = opened.iter().position(|message| message == &presence);

    assert!(open.is_some());
    assert_eq!(open.zip(held).map(|(open, held)| open < held), Some(true));
    assert_eq!(instances.gate(vec![presence.clone()]), vec![presence]);
}
