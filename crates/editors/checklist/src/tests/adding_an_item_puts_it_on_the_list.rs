use super::*;

#[test]
fn adding_an_item_puts_it_on_the_list() {
    let (mut editor, block) = editor(&[]);
    editor.record();

    editor.find("checklist.draft").click();
    editor.run();
    editor.record();
    editor.find("checklist.draft").type_text("buy milk");
    editor.run();
    editor.record();
    editor.find("checklist.add").click();
    editor.run();
    editor.record();

    assert_eq!(items(&block), [("buy milk".to_owned(), false)]);
    editor.snapshot("adding_an_item_puts_it_on_the_list");
}
