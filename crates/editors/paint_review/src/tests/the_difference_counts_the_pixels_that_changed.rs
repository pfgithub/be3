use super::*;

#[test]
fn the_difference_counts_the_pixels_that_changed() {
    let approved = egui::ColorImage::new([4, 4], vec![egui::Color32::BLACK; 16]);
    let mut current = approved.clone();
    current.pixels[5] = egui::Color32::RED;
    current.pixels[6] = egui::Color32::RED;

    let painted = crate::render::difference(&approved, &current);
    assert_eq!(painted.image.size, [4, 4]);
    assert_eq!(
        painted.description,
        "2 pixels differ, in a 2x1 region at (1, 1)"
    );

    let same = crate::render::difference(&approved, &approved);
    assert_eq!(
        same.description,
        "these frames are the same, pixel for pixel"
    );

    let taller = egui::ColorImage::new([4, 6], vec![egui::Color32::BLACK; 24]);
    let grown = crate::render::difference(&approved, &taller);
    assert_eq!(grown.image.size, [4, 6]);
    assert!(grown
        .description
        .starts_with("the painting is 4x6, it used to be 4x4; "));
}
