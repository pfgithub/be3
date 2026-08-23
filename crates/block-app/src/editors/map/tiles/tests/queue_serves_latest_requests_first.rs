use super::{RequestQueue, TileId};

#[test]
fn queue_serves_latest_requests_first() {
    let mut queue = RequestQueue::new();
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
    queue.enqueue(first);
    queue.enqueue(second);

    queue.enqueue(first);

    assert_eq!(queue.pop(), Some(first));
    assert_eq!(queue.pop(), Some(second));

    queue.enqueue(second);
    assert_eq!(queue.pop(), None);
    assert!(queue.is_empty());
}
