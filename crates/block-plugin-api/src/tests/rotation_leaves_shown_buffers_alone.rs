use super::*;

#[test]
fn rotation_leaves_shown_buffers_alone() {
    let mut rotation = SurfaceRotation::default();
    let drawn: Vec<usize> = (0..8).map(|_| rotation.advance(SURFACE_BUFFERS)).collect();
    for window in drawn.windows(3) {
        assert_eq!(
            window
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3,
            "{drawn:?} draws over a buffer the host may still be showing"
        );
    }
}
