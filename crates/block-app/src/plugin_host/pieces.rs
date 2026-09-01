use eframe::egui;

pub(super) fn subtract(area: egui::Rect, holes: &[egui::Rect]) -> Vec<egui::Rect> {
    if !area.is_positive() {
        return Vec::new();
    }
    let covering: Vec<egui::Rect> = holes
        .iter()
        .map(|hole| hole.intersect(area))
        .filter(|hole| hole.is_positive())
        .collect();
    if covering.is_empty() {
        return vec![area];
    }
    let mut columns = vec![area.min.x, area.max.x];
    let mut rows = vec![area.min.y, area.max.y];
    for hole in &covering {
        columns.push(hole.min.x);
        columns.push(hole.max.x);
        rows.push(hole.min.y);
        rows.push(hole.max.y);
    }
    let bounds = |values: &mut Vec<f32>, low: f32, high: f32| {
        values.retain(|value| *value > low && *value < high);
        values.push(low);
        values.push(high);
        values.sort_by(|left, right| left.total_cmp(right));
        values.dedup();
    };
    bounds(&mut columns, area.min.x, area.max.x);
    bounds(&mut rows, area.min.y, area.max.y);
    let mut pieces: Vec<egui::Rect> = Vec::new();
    for row in rows.windows(2) {
        let mut open: Option<egui::Rect> = None;
        for column in columns.windows(2) {
            let cell = egui::Rect::from_min_max(
                egui::pos2(column[0], row[0]),
                egui::pos2(column[1], row[1]),
            );
            if !cell.is_positive() {
                continue;
            }
            let centre = cell.center();
            if covering.iter().any(|hole| hole.contains(centre)) {
                if let Some(piece) = open.take() {
                    pieces.push(piece);
                }
                continue;
            }
            open = Some(match open {
                Some(piece) => piece.union(cell),
                None => cell,
            });
        }
        if let Some(piece) = open.take() {
            pieces.push(piece);
        }
    }
    merge_rows(pieces)
}

fn merge_rows(pieces: Vec<egui::Rect>) -> Vec<egui::Rect> {
    let mut merged: Vec<egui::Rect> = Vec::new();
    for piece in pieces {
        let joined = merged.iter_mut().find(|other| {
            other.min.x == piece.min.x && other.max.x == piece.max.x && other.max.y == piece.min.y
        });
        match joined {
            Some(other) => other.max.y = piece.max.y,
            None => merged.push(piece),
        }
    }
    merged
}

#[cfg(test)]
mod tests;
