use super::*;

#[test]
fn adding_a_heading_puts_it_on_the_canvas() {
    let (mut editor, block) = editor();

    editor.find("gui-builder.palette.Heading").click();
    editor.run();

    let builder = block.read().unwrap();
    assert_eq!(builder.widgets().len(), 1);
    drop(builder);
    editor.snapshot("adding_a_heading_puts_it_on_the_canvas");
}
