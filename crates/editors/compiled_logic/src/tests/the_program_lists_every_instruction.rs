use super::*;

#[test]
fn the_program_lists_every_instruction() {
    let (mut editor, block) = editor();

    let instructions = block.read().unwrap().program().instructions.clone();
    assert_eq!(
        instructions
            .iter()
            .map(format_instruction)
            .collect::<Vec<_>>(),
        vec!["NOT m0 -> m1", "SAVE m1 -> s0"]
    );
    editor.snapshot("the_program_lists_every_instruction");
}
