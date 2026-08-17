use super::*;

#[test]
fn queue_saturation_preserves_ordered_input() {
    let mut session = running_session();
    for index in 0..MAX_QUEUED_MESSAGES {
        session
            .enqueue(input(InputEvent::Key {
                physical: crate::PhysicalKey::Code(index as u32),
                logical: index.to_string(),
                pressed: true,
                repeat: false,
            }))
            .unwrap();
    }
    assert_eq!(
        session.enqueue(input(InputEvent::Text("overflow".into()))),
        Err(QueueError::Full)
    );
    for index in 0..MAX_QUEUED_MESSAGES {
        let Message::Input(batch) = session.next_outbound().unwrap() else {
            panic!()
        };
        let InputEvent::Key { logical, .. } = &batch.events[0] else {
            panic!()
        };
        assert_eq!(logical, &index.to_string());
    }
}
