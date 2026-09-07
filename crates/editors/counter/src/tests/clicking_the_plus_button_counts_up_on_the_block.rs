use super::*;

#[test]
fn clicking_the_plus_button_counts_up_on_the_block() {
    let (mut editor, block) = editor();

    click(&mut editor, |app| app.ui().unwrap().buttons().increment);
    click(&mut editor, |app| app.ui().unwrap().buttons().increment);

    assert_eq!(block.read().unwrap().count(), 2);
    assert_eq!(shown(&mut editor), "\"2\"");
    editor.snapshot("clicking_the_plus_button_counts_up_on_the_block");
}
