use super::*;

#[test]
fn the_painting_can_be_zoomed_in_on() {
    let (_review, mut editor) = Review::open();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    let fitted = editor.app().zoom();
    editor.record();

    for _ in 0..4 {
        editor.find("paint_review.zoom.in").click();
        editor.run();
    }
    assert!(editor.app().zoom() > fitted);
    editor.record();

    editor.find("paint_review.zoom.out").click();
    editor.run();
    editor.record();

    editor.find("paint_review.zoom.actual").click();
    editor.run();
    assert_eq!(editor.app().zoom(), 1.0);

    editor.find("paint_review.zoom.fit").click();
    editor.run();
    assert_eq!(editor.app().zoom(), fitted);
    editor.record();

    editor.snapshot("zooming_into_a_painting");
}
