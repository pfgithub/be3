use std::collections::BTreeMap;

use crate::{Content, Primitive, Snapshot, Texture, Triangle, Vertex};

mod a_painting_is_the_same_however_the_atlas_was_packed;
mod a_snapshot_survives_a_round_trip;
mod a_triangle_is_filled_with_its_corner_colour;

fn white() -> Texture {
    Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap()
}

fn triangle(colour: [u8; 4]) -> Snapshot {
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: colour,
    };
    Snapshot {
        size: [8, 8],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 255],
        primitives: vec![Primitive {
            clip: [0.0, 0.0, 8.0, 8.0],
            content: Content::Mesh(vec![Triangle {
                texture: 0,
                corners: [corner(0.0, 0.0), corner(8.0, 0.0), corner(0.0, 8.0)],
            }]),
        }],
        textures: BTreeMap::from([(0, white())]),
    }
}

fn painted(frames: &[&str]) -> Vec<u8> {
    let context = egui::Context::default();
    let mut textures = crate::TextureStore::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(200.0, 60.0),
        )),
        ..Default::default()
    };
    let mut bytes = Vec::new();
    for text in frames {
        let output = context.run_ui(input.clone(), |ui| {
            ui.label(*text);
        });
        textures.apply(&output.textures_delta);
        bytes = crate::capture(&context, &output, &textures)
            .unwrap()
            .encode()
            .unwrap();
    }
    bytes
}
