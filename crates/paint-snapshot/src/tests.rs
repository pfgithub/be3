use std::collections::BTreeMap;

use crate::{Content, Mesh, Primitive, Snapshot, Texture, Vertex};

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
            content: Content::Mesh(Mesh {
                texture: 0,
                indices: vec![0, 1, 2],
                vertices: vec![corner(0.0, 0.0), corner(8.0, 0.0), corner(0.0, 8.0)],
            }),
        }],
        textures: BTreeMap::from([(0, white())]),
    }
}
