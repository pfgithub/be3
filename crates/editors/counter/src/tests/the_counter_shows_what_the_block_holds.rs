use super::*;
use block_client::blocks::counter::CounterOperation;

#[test]
fn the_counter_shows_what_the_block_holds() {
    let (mut editor, block) = editor();

    for _ in 0..3 {
        block.operate(CounterOperation::Increment);
    }
    editor.run();

    assert_eq!(shown(&mut editor), "\"3\"");
}
