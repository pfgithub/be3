mod support;

use block::ServerMessage;
use support::{create, read, relationships, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn reparents_without_changing_either_parents_references() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let first_parent = Uuid::new_v4();
    let second_parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    for (id, references) in [
        (child, vec![]),
        (first_parent, vec![child]),
        (second_parent, vec![child]),
    ] {
        assert!(matches!(
            create(&mut socket, id, references).await,
            ServerMessage::Ok { .. }
        ));
    }
    assert!(matches!(
        set_parent(&mut socket, child, Some(first_parent)).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, Some(second_parent)).await,
        ServerMessage::Ok { .. }
    ));

    let mut expected_backrefs = vec![first_parent, second_parent];
    expected_backrefs.sort_unstable();
    assert_eq!(
        relationships(read(&mut socket, child).await),
        (Some(second_parent), vec![], expected_backrefs)
    );
    assert_eq!(
        relationships(read(&mut socket, first_parent).await).1,
        vec![child]
    );
    assert_eq!(
        relationships(read(&mut socket, second_parent).await).1,
        vec![child]
    );
    server.cleanup().await;
}
