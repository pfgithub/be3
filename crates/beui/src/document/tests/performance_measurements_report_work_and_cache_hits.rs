use super::*;
use crate::reactive::{build, view, Fill};

#[test]
fn performance_measurements_report_work_and_cache_hits() {
    let document = build(|| view! { <Fill color=Color32::WHITE radius=0 /> });
    let mut harness = Harness::new(document);

    assert_eq!(harness.document().performance().samples, 0);
    harness.frame(Vec::new());

    let first = harness.document().performance();
    assert_eq!(first.samples, 1);
    assert_eq!(first.latest.layout_passes, 1);
    assert!(first.latest.painted);
    assert_eq!(first.latest.nodes, 1);
    assert_eq!(first.latest.shapes, 1);
    assert_eq!(first.layout_cache_hits, 0);
    assert_eq!(first.paint_cache_hits, 0);

    harness.frame(Vec::new());

    let cached = harness.document().performance();
    assert_eq!(cached.samples, 2);
    assert_eq!(cached.latest.layout_passes, 0);
    assert!(!cached.latest.painted);
    assert_eq!(cached.layout_cache_hits, 1);
    assert_eq!(cached.paint_cache_hits, 1);
    assert!(cached.peak.total >= cached.average.total);

    harness.document_mut().reset_performance();
    assert_eq!(harness.document().performance().samples, 0);
}
