use super::super::*;

#[test]
fn eraser_splits_stroke_at_hit_segment() {
    let stroke = Stroke {
        tool: Tool::Pen,
        points: vec![
            Vector::new(0.0, 0.0),
            Vector::new(10.0, 0.0),
            Vector::new(20.0, 0.0),
            Vector::new(30.0, 0.0),
            Vector::new(40.0, 0.0),
        ],
    };

    let strokes = stroke.split_away_from(Vector::new(20.0, 0.0), 1.0);

    assert_eq!(strokes.len(), 2);
    assert_eq!(strokes[0].points.len(), 2);
    assert_eq!(strokes[1].points.len(), 2);
    assert_eq!(strokes[0].points[0][0], 0.0);
    assert_eq!(strokes[1].points[1][0], 40.0);
}
