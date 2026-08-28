use crate::format::{Content, Frame, Snapshot};

pub struct Difference {
    pub description: String,
    pub frame: Option<usize>,
}

pub fn difference(before: &Snapshot, after: &Snapshot) -> Option<Difference> {
    if before == after {
        return None;
    }
    if before.frames.len() != after.frames.len() {
        return Some(Difference {
            description: format!(
                "the recording is {} long, it used to be {}",
                frames(after.frames.len()),
                frames(before.frames.len())
            ),
            frame: None,
        });
    }
    let changed = before
        .frames
        .iter()
        .zip(&after.frames)
        .position(|(before, after)| before != after);
    let Some(index) = changed else {
        return Some(Difference {
            description: "the textures the painting uses changed".to_owned(),
            frame: None,
        });
    };
    let description = within(&before.frames[index], &after.frames[index]);
    Some(Difference {
        description: match after.frames.len() {
            1 => description,
            count => format!("frame {} of {count} changed: {description}", index + 1),
        },
        frame: Some(index),
    })
}

fn frames(count: usize) -> String {
    match count {
        1 => "one frame".to_owned(),
        count => format!("{count} frames"),
    }
}

fn within(before: &Frame, after: &Frame) -> String {
    if before.size != after.size {
        return format!(
            "the painting is {}x{}, it used to be {}x{}",
            after.size[0], after.size[1], before.size[0], before.size[1]
        );
    }
    if before.background != after.background {
        return format!(
            "the background is {:?}, it used to be {:?}",
            after.background, before.background
        );
    }
    if before.primitives.len() != after.primitives.len() {
        return format!(
            "the painting has {} draw calls, it used to have {}",
            after.primitives.len(),
            before.primitives.len()
        );
    }
    let index = before
        .primitives
        .iter()
        .zip(&after.primitives)
        .position(|(before, after)| before != after);
    match index {
        Some(index) => format!(
            "draw call {index} changed: {}",
            changes(
                &before.primitives[index].content,
                &after.primitives[index].content
            )
        ),
        None => "the painting is scaled differently".to_owned(),
    }
}

fn changes(before: &Content, after: &Content) -> String {
    let replaced = format!("{} became {}", summary(before), summary(after));
    let (Content::Mesh(before), Content::Mesh(after)) = (before, after) else {
        return replaced;
    };
    if before.len() != after.len() {
        return replaced;
    }

    let moved: Vec<[f32; 2]> = before
        .iter()
        .zip(after)
        .flat_map(|(before, after)| before.corners.into_iter().zip(after.corners))
        .filter(|(before, after)| before != after)
        .map(|(_, after)| after.pos)
        .collect();
    if moved.is_empty() {
        return "the triangles sample different textures".to_owned();
    }

    let region = moved
        .iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |region, point| {
            [
                region[0].min(point[0]),
                region[1].min(point[1]),
                region[2].max(point[0]),
                region[3].max(point[1]),
            ]
        });
    format!(
        "{} of {} corners moved or changed colour, between ({}, {}) and ({}, {})",
        moved.len(),
        after.len() * 3,
        region[0].round(),
        region[1].round(),
        region[2].round(),
        region[3].round()
    )
}

fn summary(content: &Content) -> String {
    match content {
        Content::Mesh(triangles) => format!(
            "a mesh of {} triangles on {} textures",
            triangles.len(),
            triangles
                .iter()
                .map(|triangle| triangle.texture)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        ),
        Content::Callback(rect) => format!("a callback over {rect:?}"),
    }
}
