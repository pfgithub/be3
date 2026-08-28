use block_editor_plugin::egui::{Color32, ColorImage};

use super::Painted;

const HIGHLIGHT: [f32; 3] = [255.0, 59.0, 92.0];
const MISSING: Color32 = Color32::from_rgb(96, 16, 34);
const PAPER: f32 = 236.0;
const GHOST: f32 = 0.22;
const TINT: f32 = 0.55;

pub fn difference(approved: &ColorImage, current: &ColorImage) -> Painted {
    let width = approved.size[0].max(current.size[0]);
    let height = approved.size[1].max(current.size[1]);
    let mut pixels = Vec::with_capacity(width * height);
    let mut changed = 0usize;
    let mut region: Option<[usize; 4]> = None;

    for y in 0..height {
        for x in 0..width {
            let before = at(approved, x, y);
            let after = at(current, x, y);
            let pixel = match (before, after) {
                (Some(before), Some(after)) if before == after => ghost(after),
                (None, None) => MISSING,
                (_, Some(after)) => tinted(after),
                (Some(_), None) => MISSING,
            };
            if before != after {
                changed += 1;
                region = Some(match region {
                    None => [x, y, x, y],
                    Some(held) => [
                        held[0].min(x),
                        held[1].min(y),
                        held[2].max(x),
                        held[3].max(y),
                    ],
                });
            }
            pixels.push(pixel);
        }
    }

    Painted {
        image: ColorImage::new([width, height], pixels),
        description: describe(approved, current, changed, region),
    }
}

fn describe(
    approved: &ColorImage,
    current: &ColorImage,
    changed: usize,
    region: Option<[usize; 4]>,
) -> String {
    let resized = (approved.size != current.size).then(|| {
        format!(
            "the painting is {}x{}, it used to be {}x{}; ",
            current.size[0], current.size[1], approved.size[0], approved.size[1]
        )
    });
    let Some([left, top, right, bottom]) = region else {
        return "these frames are the same, pixel for pixel".to_owned();
    };
    format!(
        "{}{changed} {} differ, in a {}x{} region at ({left}, {top})",
        resized.unwrap_or_default(),
        if changed == 1 { "pixel" } else { "pixels" },
        right - left + 1,
        bottom - top + 1,
    )
}

fn at(image: &ColorImage, x: usize, y: usize) -> Option<Color32> {
    if x >= image.size[0] || y >= image.size[1] {
        return None;
    }
    image.pixels.get(y * image.size[0] + x).copied()
}

fn ghost(pixel: Color32) -> Color32 {
    let luma = 0.299 * pixel.r() as f32 + 0.587 * pixel.g() as f32 + 0.114 * pixel.b() as f32;
    let value = PAPER + (luma - PAPER) * GHOST;
    Color32::from_gray(value.round().clamp(0.0, 255.0) as u8)
}

fn tinted(pixel: Color32) -> Color32 {
    let channels = [pixel.r(), pixel.g(), pixel.b()];
    let mixed = channels
        .iter()
        .zip(HIGHLIGHT)
        .map(|(channel, highlight)| {
            (*channel as f32 * (1.0 - TINT) + highlight * TINT)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect::<Vec<u8>>();
    Color32::from_rgb(mixed[0], mixed[1], mixed[2])
}
