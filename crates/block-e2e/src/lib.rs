#[cfg(test)]
mod tests {
    use std::{env, future::Future, time::Duration};

    use block::Block;
    use block_client::BlockClient;
    use serde::{Deserialize, Serialize};
    use tokio::{fs, net::TcpListener};
    use uuid::Uuid;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct Counter {
        count: i64,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    enum CounterOperation {
        Add(i64),
    }

    impl Block for Counter {
        type Operation = CounterOperation;
        const TYPE_ID: Uuid = Uuid::from_u128(1);

        fn apply_operation(block: &mut Self, operation: &Self::Operation) {
            let CounterOperation::Add(amount) = operation;
            block.count += amount;
        }

        fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}
    }

    #[tokio::test]
    async fn real_clients_synchronize_through_the_real_server() {
        let data_dir = env::temp_dir().join(format!("block-e2e-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_data_dir = data_dir.clone();
        let server = tokio::spawn(async move {
            block_server::serve(listener, server_data_dir)
                .await
                .unwrap();
        });
        let url = format!("ws://{address}");

        let client_a = BlockClient::new();
        client_a.connect(url.clone());
        let block_a = client_a.create_block(Counter { count: 0 });
        let block_id = block_a.id();
        timeout(block_a.loaded()).await;
        block_a.operate(CounterOperation::Add(1));
        timeout(client_a.synchronized()).await;
        assert_eq!(block_a.read().unwrap().count, 1);

        let client_b = BlockClient::new();
        client_b.connect(url.clone());
        let block_b = client_b.get_block::<Counter>(block_id);
        assert!(block_b.read().is_none());
        timeout(block_b.loaded()).await;
        assert_eq!(block_b.read().unwrap().count, 1);

        block_b.operate(CounterOperation::Add(10));
        timeout(client_b.synchronized()).await;
        block_a.operate(CounterOperation::Add(100));
        timeout(client_a.synchronized()).await;
        assert_eq!(block_a.read().unwrap().count, 111);

        block_b.operate(CounterOperation::Add(1_000));
        timeout(client_b.synchronized()).await;
        assert_eq!(block_b.read().unwrap().count, 1_111);

        let client_c = BlockClient::new();
        client_c.connect(url);
        let block_c = client_c.get_block::<Counter>(block_id);
        timeout(block_c.loaded()).await;
        assert_eq!(block_c.read().unwrap().count, 1_111);
        timeout(client_c.synchronized()).await;

        drop(block_a);
        drop(block_b);
        drop(block_c);
        drop(client_a);
        drop(client_b);
        drop(client_c);
        server.abort();
        let _ = server.await;
        fs::remove_dir_all(data_dir).await.unwrap();
    }

    async fn timeout(future: impl Future<Output = ()>) {
        tokio::time::timeout(Duration::from_secs(2), future)
            .await
            .unwrap();
    }
}
