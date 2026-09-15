use super::*;

#[test]
fn painting_skips_the_elements_outside_the_damaged_region() {
    let panels = stacked_panels();
    let (upper, lower) = (panels.upper, panels.lower);
    let mut document = panels.document;
    let (_, paints) = counted(&mut document, upper);
    let mut harness = Harness::new(document);
    let shapes = harness.frame(Vec::new()).shapes().len();
    let settled = paints.get();

    harness
        .document_mut()
        .set_frame_color(lower, Color32::from_gray(90));
    let output = harness.frame(Vec::new());

    assert_eq!(output.damage(), Some(harness.rect(lower)));
    assert_eq!(
        paints.get(),
        settled,
        "an element the damaged region does not reach must not be painted again"
    );
    assert_eq!(
        output.shapes().len(),
        shapes,
        "the shapes of a skipped element are still reported for the frame"
    );
}
