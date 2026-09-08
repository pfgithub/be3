use super::*;

#[test]
fn clicking_the_plus_button_counts_up_on_the_block() {
    let (mut editor, block) = editor();

    editor.click("counter.increment");
    editor.run();
    editor.click("counter.increment");
    editor.run();

    assert_eq!(block.read().unwrap().count(), 2);
    assert_eq!(shown(&mut editor), "\"2\"");
    editor.snapshot("clicking_the_plus_button_counts_up_on_the_block");
}
