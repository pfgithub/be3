use super::*;

#[test]
fn adding_an_item_puts_it_on_the_list() {
    let (mut editor, block) = editor(&[]);

    editor.click("checklist.draft");
    editor.run();
    editor.text("buy milk");
    editor.run();
    editor.click("checklist.add");
    editor.run();

    assert_eq!(items(&block), [("buy milk".to_owned(), false)]);
    editor.snapshot("adding_an_item_puts_it_on_the_list");
}
