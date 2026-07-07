use super::super::*;

#[test]
fn eraser_waits_for_press() {
    let mut app = NotesApp::new();
    app.selected_tool = Tool::Eraser;
    app.strokes.push(Stroke {
        tool: Tool::Pen,
        points: vec![Vector::new(160.0, 160.0), Vector::new(220.0, 160.0)],
    });

    let changed = app.pointer_moved(Vector::new(900.0, 520.0), Vector::new(180.0, 160.0));

    assert!(!changed);
    assert_eq!(app.strokes.len(), 1);
    assert_eq!(app.strokes[0].points.len(), 2);
}
