use super::*;

#[test]
fn idle_frames_do_not_read_the_count_again() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Counter::default());
    let mut changes = CountChanges::new(block.clone(), block_editor_plugin::Waker::default());

    assert_eq!(changes.take(), Some(0));
    for _ in 0..3 {
        assert_eq!(changes.take(), None);
    }

    block.operate(CounterOperation::Increment);
    block.operate(CounterOperation::Increment);
    assert_eq!(changes.take(), Some(2));
    assert_eq!(changes.take(), None);

    block.operate(CounterOperation::Decrement);
    assert_eq!(changes.take(), Some(1));
    assert_eq!(changes.take(), None);
}
