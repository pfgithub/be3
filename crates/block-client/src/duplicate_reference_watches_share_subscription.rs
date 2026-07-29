use super::*;

#[test]
fn duplicate_reference_watches_share_subscription() {
    let client = BlockClient::new();
    let first = client.watch_references(BlockReferenceList::Roots);
    let second = client.watch_references(BlockReferenceList::Roots);

    assert!(Arc::ptr_eq(&first.shared, &second.shared));
    assert_eq!(first.shared.subscribers.load(Ordering::Relaxed), 2);

    drop(first);
    assert_eq!(second.shared.subscribers.load(Ordering::Relaxed), 1);
    assert!(client
        .watched_reference_lists
        .read()
        .contains_key(&BlockReferenceList::Roots));

    drop(second);
    assert!(!client
        .watched_reference_lists
        .read()
        .contains_key(&BlockReferenceList::Roots));
}
