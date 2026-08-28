use std::collections::BTreeMap;

use crate::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};

mod a_frame_that_changed_is_named_by_its_number;
mod a_painting_is_the_same_however_the_atlas_was_packed;
mod a_recording_keeps_the_frames_it_was_given;
mod a_snapshot_survives_a_round_trip;
mod a_triangle_is_filled_with_its_corner_colour;

fn white() -> Texture {
    Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap()
}

fn triangle(colour: [u8; 4]) -> Snapshot {
    Snapshot::of(frame(colour), BTreeMap::from([(0, white())]))
}

fn frame(colour: [u8; 4]) -> Frame {
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: colour,
    };
    Frame {
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
    }
}

fn triangles(colours: &[[u8; 4]]) -> Snapshot {
    Snapshot {
        frames: colours.iter().map(|colour| frame(*colour)).collect(),
        textures: BTreeMap::from([(0, white())]),
    }
}

fn captured(frames: &[&str]) -> Vec<Snapshot> {
    let context = egui::Context::default();
    let mut textures = crate::TextureStore::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(200.0, 60.0),
        )),
        ..Default::default()
    };
    let mut captured = Vec::new();
    for text in frames {
        let output = context.run_ui(input.clone(), |ui| {
            ui.label(*text);
        });
        textures.apply(&output.textures_delta);
        captured.push(crate::capture(&context, &output, &textures).unwrap());
    }
    captured
}

fn painted(frames: &[&str]) -> Vec<u8> {
    captured(frames).pop().unwrap().encode().unwrap()
}

fn recorded(frames: &[&str]) -> Snapshot {
    let mut captured = captured(frames).into_iter();
    let mut recording = captured.next().expect("a recording needs a frame");
    for frame in captured {
        recording.append(frame);
    }
    recording
}
