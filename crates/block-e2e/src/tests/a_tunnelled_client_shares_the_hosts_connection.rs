use super::*;

#[tokio::test]
async fn a_tunnelled_client_shares_the_hosts_connection() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_data_dir = data_dir.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_data_dir)
            .await
            .unwrap();
    });
    let url = format!("http://{address}");
    let (account_id, token, workspace_id) = test_identity(&url).await;

    let host = BlockClient::new(account_id, workspace_id);
    host.connect(url, token);
    let host_block = host.create_block(Counter { count: 0 });
    let block_id = host_block.id();
    timeout(host_block.loaded()).await;

    let (endpoint, carrier) = block_client::tunnel_channel();
    let guest = BlockClient::tunneled(account_id, workspace_id, endpoint, || {});
    let pump = tokio::spawn(carry(host.open_tunnel(|| {}), carrier));

    let guest_block = guest.get_block::<Counter>(block_id);
    timeout(guest_block.loaded()).await;
    assert_eq!(guest_block.read().unwrap().count, 0);

    guest_block.operate(CounterOperation::Add(5));
    timeout(guest.synchronized()).await;
    timeout(host_block.wait_until(|counter| counter.count == 5)).await;

    host_block.operate(CounterOperation::Add(2));
    timeout(host.synchronized()).await;
    timeout(guest_block.wait_until(|counter| counter.count == 7)).await;

    drop(guest_block);
    drop(guest);
    pump.abort();
    let _ = pump.await;
    drop(host_block);
    drop(host);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}

/// Stands in for the plugin host, which does this from its frame loop.
async fn carry(mut tunnel: block_client::Tunnel, mut carrier: block_client::TunnelCarrier) {
    loop {
        tokio::select! {
            message = tunnel.recv() => match message {
                Some(text) => carrier.send(text),
                None => return,
            },
            message = carrier.recv() => match message {
                Some(text) => tunnel.send(text),
                None => return,
            },
        }
    }
}
