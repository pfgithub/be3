use crate::{crypto::AccountKeypair, ManagementClient};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn management_client_round_trips_device_key_transfer() {
    let root = std::env::temp_dir().join(format!("block-client-key-transfer-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });
    let client = ManagementClient::new(url).unwrap();

    let device_a_keypair = AccountKeypair::generate();
    let device_a = client
        .register(
            "linked@example.com",
            "Linked",
            "linked-password",
            &device_a_keypair,
        )
        .await
        .unwrap();

    // A second device logs in with a throwaway candidate key. The account
    // already has an identity key, so the candidate is not adopted and this
    // device has no usable private key of its own.
    let device_b = client
        .login(
            "linked@example.com",
            "linked-password",
            AccountKeypair::generate().public_key().to_bytes().to_vec(),
        )
        .await
        .unwrap();
    assert_eq!(device_b.account.public_key, device_a.account.public_key);

    let ephemeral = AccountKeypair::generate();
    client
        .request_key_transfer(&device_b.token, ephemeral.public_key().to_bytes().to_vec())
        .await
        .unwrap();

    // Device A notices the pending request and fulfils it by sealing its
    // real account keypair to the ephemeral public key.
    let pending = client
        .list_key_transfer_request(&device_a.token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending, ephemeral.public_key().to_bytes().to_vec());
    let wrapped = device_a_keypair.seal_private_key_for(&ephemeral.public_key());
    client
        .submit_key_transfer(&device_a.token, wrapped)
        .await
        .unwrap();
    assert_eq!(
        client
            .list_key_transfer_request(&device_a.token)
            .await
            .unwrap(),
        None
    );

    // Device B collects and unseals it, ending up with the exact same
    // account identity as device A -- and the transfer is single-use.
    let wrapped = client
        .poll_key_transfer(&device_b.token)
        .await
        .unwrap()
        .unwrap();
    let recovered = ephemeral.unseal_private_key(&wrapped).unwrap();
    assert_eq!(
        recovered.public_key().to_bytes(),
        device_a_keypair.public_key().to_bytes()
    );
    assert_eq!(
        client.poll_key_transfer(&device_b.token).await.unwrap(),
        None
    );

    server.abort();
    let _ = server.await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
