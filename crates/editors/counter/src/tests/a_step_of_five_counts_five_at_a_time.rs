use super::*;

#[test]
fn a_step_of_five_counts_five_at_a_time() {
    let (mut editor, block) = editor();

    editor.find("counter.step.5").click();
    editor.run();
    editor.find("counter.increment").click();
    editor.run();

    assert_eq!(block.read().unwrap().count(), 5);
    editor.snapshot("a_step_of_five_counts_five_at_a_time");
}
