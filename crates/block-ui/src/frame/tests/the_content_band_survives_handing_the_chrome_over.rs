use super::*;

#[test]
fn the_content_band_survives_handing_the_chrome_over() {
    let size = egui::vec2(1200.0, 800.0);
    let context = egui::Context::default();
    let mut drawn = FrameOutcome::default();
    let mut reserved = FrameOutcome::default();
    for pass in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..egui::RawInput::default()
        };
        let mut bands = Bands::default();
        let _ = context.run_ui(input, |ui| {
            let chrome = match pass < 2 {
                true => Chrome::Drawn,
                false => Chrome::Reserved,
            };
            let outcome = frame().chrome(chrome).show(ui, &mut bands);
            match pass < 2 {
                true => drawn = outcome,
                false => reserved = outcome,
            }
        });
    }
    assert!(drawn.rects.toolbar.is_some());
    assert!(drawn.rects.left_sidebar.is_some());
    assert!(drawn.rects.right_sidebar.is_some());
    assert_eq!(reserved.rects.content_band, drawn.rects.content_band);
    assert_eq!(reserved.rects.toolbar, drawn.rects.toolbar);
    assert_eq!(reserved.rects.left_sidebar, drawn.rects.left_sidebar);
    assert_eq!(reserved.rects.right_sidebar, drawn.rects.right_sidebar);
}
