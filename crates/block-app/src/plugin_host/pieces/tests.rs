use super::*;

fn rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

fn area(pieces: &[egui::Rect]) -> f32 {
    pieces
        .iter()
        .map(|piece| piece.width() * piece.height())
        .sum()
}

fn disjoint(pieces: &[egui::Rect]) -> bool {
    pieces.iter().enumerate().all(|(index, piece)| {
        pieces[index + 1..]
            .iter()
            .all(|other| !piece.intersect(*other).is_positive())
    })
}

mod a_hole_in_the_middle_leaves_a_ring;
mod nothing_is_left_when_a_hole_covers_everything;
mod pieces_of_two_holes_stay_disjoint;
