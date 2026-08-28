use super::*;

#[test]
fn a_painting_is_only_rastered_once() {
    let (review, mut editor) = Review::open();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    assert_eq!(editor.app().rasters(), 1);

    editor.find("paint_review.approve").click();
    editor.run();
    assert_eq!(editor.app().rasters(), 1);

    review.write(PATH, &painting(200));
    editor.find("paint_review.refresh").click();
    editor.run();
    assert_eq!(editor.app().rasters(), 2);

    editor.find("paint_review.view.approved").click();
    editor.run();
    editor.find("paint_review.view.current").click();
    editor.run();
    editor.find("paint_review.view.approved").click();
    editor.run();
    assert_eq!(editor.app().rasters(), 2);
}
