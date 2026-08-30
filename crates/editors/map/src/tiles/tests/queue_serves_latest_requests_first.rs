use super::*;

#[test]
fn queue_serves_latest_requests_first() {
    let mut worker = TileWorker::spawn(Waker::default());
    let first = TileId {
        zoom: 1,
        x: 0,
        y: 0,
    };
    let second = TileId {
        zoom: 1,
        x: 1,
        y: 0,
    };
    worker.request(first);
    worker.request(second);

    worker.request(first);

    assert_eq!(worker.queued, vec![first, second]);
    assert!(worker.requested.contains(&first));
}
