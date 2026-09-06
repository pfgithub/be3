use super::*;

#[test]
fn resetting_puts_the_block_back_to_zero() {
    let (mut editor, block) = editor();

    click(&mut editor, |app| app.demo().unwrap().buttons().increment);
    click(&mut editor, |app| app.demo().unwrap().buttons().decrement);
    click(&mut editor, |app| app.demo().unwrap().buttons().decrement);
    assert_eq!(block.read().unwrap().count(), -1);

    click(&mut editor, |app| app.demo().unwrap().buttons().reset);

    assert_eq!(block.read().unwrap().count(), 0);
    assert_eq!(shown(&mut editor), "\"0\"");
}
