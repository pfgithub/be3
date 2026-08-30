use super::*;

use crate::render::{Painted, Paintings};

const SIZE: [usize; 2] = [2048, 2048];
const FRAMES: usize = 2;

#[test]
fn a_painting_being_shown_is_not_evicted_for_another() {
    let context = egui::Context::default();
    let mut paintings = Paintings::default();
    paintings.keep(vec!["shown".to_owned()]);

    hold(&mut paintings, &context, "shown", 30);
    hold(&mut paintings, &context, "other", 200);

    assert!(paintings.rendered(&context, "shown", 0).is_some());
    assert!(paintings.rendered(&context, "other", 0).is_none());
}

fn hold(paintings: &mut Paintings, context: &egui::Context, hash: &str, shade: u8) {
    for frame in 0..FRAMES {
        assert!(paintings
            .computed(context, hash, frame, FRAMES, || Ok(large(shade)))
            .is_ok());
    }
}

fn large(shade: u8) -> Painted {
    let pixels = vec![egui::Color32::from_gray(shade); SIZE[0] * SIZE[1]];
    Painted {
        image: egui::ColorImage::new(SIZE, pixels),
        description: format!("a painting too large to hold beside another, shaded {shade}"),
    }
}
