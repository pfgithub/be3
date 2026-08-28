use super::*;

#[test]
fn choosing_a_painting_rasters_the_one_it_was_approved_as() {
    let (review, mut editor) = Review::open();
    review.approve(PATH, &painting(90));
    review.write(PATH, &recording(&[30, 200]));
    editor.find("paint_review.refresh").click();
    editor.run();
    assert_eq!(status(&mut editor, PATH), Some(Status::Modified));
    assert_eq!(editor.app().rasters(), 0);

    editor.find(&entry_id(PATH)).click();
    editor.run();
    assert_eq!(editor.app().rasters(), 3);

    editor.find("paint_review.frame.next").click();
    editor.run();
    editor.find("paint_review.view.approved").click();
    editor.run();
    editor.find("paint_review.view.current").click();
    editor.run();
    assert_eq!(editor.app().rasters(), 3);
}
