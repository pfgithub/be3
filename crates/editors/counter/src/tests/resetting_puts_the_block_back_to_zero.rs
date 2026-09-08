use super::*;

#[test]
fn resetting_puts_the_block_back_to_zero() {
    let (mut editor, block) = editor();

    editor.click("counter.increment");
    editor.run();
    editor.click("counter.decrement");
    editor.run();
    editor.click("counter.decrement");
    editor.run();
    assert_eq!(block.read().unwrap().count(), -1);

    editor.click("counter.reset");
    editor.run();

    assert_eq!(block.read().unwrap().count(), 0);
    assert_eq!(shown(&mut editor), "\"0\"");
}
