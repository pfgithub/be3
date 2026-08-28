use super::*;

#[test]
fn filtering_to_open_hides_the_items_that_are_done() {
    let (mut editor, block) = editor(&[("buy milk", false), ("call the vet", false)]);

    editor.find("checklist.item.1.done").click();
    editor.run();
    editor.find("checklist.filter.open").click();
    editor.run();

    assert_eq!(
        items(&block),
        [
            ("buy milk".to_owned(), false),
            ("call the vet".to_owned(), true)
        ]
    );
    editor.snapshot("filtering_to_open_hides_the_items_that_are_done");
}
