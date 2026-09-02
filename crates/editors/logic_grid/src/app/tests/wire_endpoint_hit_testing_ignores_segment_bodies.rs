use super::*;

#[test]
fn wire_endpoint_hit_testing_ignores_segment_bodies() {
    let horizontal = wire((0, 0), (10, 0), 1);
    let vertical = wire((5, -5), (5, 5), 1);
    assert_eq!(
        nearest_wire_endpoint(&[vertical, horizontal], [0.5, 0.5], 0.1),
        Some(WireEndpoint {
            wire: horizontal,
            end: WireEnd::Start
        })
    );
    assert_eq!(nearest_wire_endpoint(&[horizontal], [5.0, 0.5], 0.6), None);
}
