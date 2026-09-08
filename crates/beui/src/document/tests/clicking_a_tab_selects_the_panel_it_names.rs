use super::*;

#[test]
fn clicking_a_tab_selects_the_panel_it_names() {
    let mut document = Document::new();
    let tabs = styled::tabs(&mut document, &["List", "Load"], 0);
    let reported = Rc::new(Cell::new(0));
    let sink = reported.clone();
    styled::set_tabs_on_change(&mut document, tabs, move |_document, selected| {
        sink.set(selected);
    });
    toolbar(&mut document, &[tabs]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);

    let root = harness.document().shadow_root(tabs);
    let row = harness.document().shadow_root(root);
    let second = harness.document().children(row)[1];
    harness.click(harness.center(second));
    harness.frame(Vec::new());

    assert_eq!(styled::tabs_selected(harness.document(), tabs), 1);
    assert_eq!(reported.get(), 1);
}
