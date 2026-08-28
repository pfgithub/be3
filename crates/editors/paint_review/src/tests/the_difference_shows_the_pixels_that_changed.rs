use super::*;

#[test]
fn the_difference_shows_the_pixels_that_changed() {
    let (review, mut editor) = Review::open();
    review.approve(PATH, &marked(30, 3.0));
    review.write(PATH, &marked(30, 12.0));
    editor.find("paint_review.refresh").click();
    editor.run();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    editor.record();

    editor.find("paint_review.view.difference").click();
    editor.run();
    let rastered = editor.app().rasters();
    editor.record();

    editor.find("paint_review.view.side_by_side").click();
    editor.run();
    editor.record();
    assert_eq!(editor.app().rasters(), rastered);

    editor.find("paint_review.view.approved").click();
    editor.run();
    editor.record();
    assert_eq!(editor.app().rasters(), rastered);

    editor.snapshot("comparing_a_painting_with_the_one_it_was_approved_as");
}
