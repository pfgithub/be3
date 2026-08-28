use super::*;

#[test]
fn a_recording_is_reviewed_one_frame_at_a_time() {
    let (review, mut editor) = Review::open();
    review.write(PATH, &recording(&[20, 130, 240]));
    editor.find("paint_review.refresh").click();
    editor.run();
    editor.find(&entry_id(PATH)).click();
    editor.run();
    assert_eq!(editor.app().frame(), 0);
    assert_eq!(editor.app().rasters(), 3);
    editor.record();

    editor.find("paint_review.frame.next").click();
    editor.run();
    assert_eq!(editor.app().frame(), 1);
    assert_eq!(editor.app().rasters(), 3);
    editor.record();

    editor.find("paint_review.frame.next").click();
    editor.run();
    editor.find("paint_review.frame.previous").click();
    editor.run();
    assert_eq!(editor.app().frame(), 1);
    editor.record();

    editor.find("paint_review.frame.play").click();
    editor.step();
    let started = editor.app().frame();
    editor.step();
    editor.step();
    assert_ne!(editor.app().frame(), started);
    editor.record();

    editor.find("paint_review.frame.play").click();
    editor.step();
    let paused = editor.app().frame();
    editor.step();
    editor.step();
    assert_eq!(editor.app().frame(), paused);

    editor.snapshot("stepping_through_a_recording");
}
