#[cfg(test)]
mod tests {
    use std::{env, future::Future, time::Duration};

    use block::Block;
    use block_client::blocks::text::TextDocument;
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
        type History = block::NoHistory;
        const TYPE_ID: Uuid = Uuid::from_u128(1);

        fn apply_operation(block: &mut Self, operation: &Self::Operation) {
            let CounterOperation::Add(amount) = operation;
            block.count += amount;
        }

        fn implicit_name(&self) -> String {
            format!("Counter {}", self.count)
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

        let client_a = BlockClient::new(Uuid::new_v4());
        client_a.connect(url.clone());
        let block_a = client_a.create_block(Counter { count: 0 });
        let block_id = block_a.id();
        timeout(block_a.loaded()).await;
        block_a.operate(CounterOperation::Add(1));
        timeout(client_a.synchronized()).await;
        assert_eq!(block_a.read().unwrap().count, 1);

        let client_b = BlockClient::new(Uuid::new_v4());
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

        let client_c = BlockClient::new(Uuid::new_v4());
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

    #[tokio::test]
    async fn batched_updates_are_observed_together() {
        let data_dir = env::temp_dir().join(format!("block-e2e-batch-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_data_dir = data_dir.clone();
        let server = tokio::spawn(async move {
            block_server::serve(listener, server_data_dir)
                .await
                .unwrap();
        });
        let url = format!("ws://{address}");

        let client_a = BlockClient::new(Uuid::new_v4());
        client_a.connect(url.clone());
        let first_a = client_a.create_block(Counter { count: 0 });
        let second_a = client_a.create_block(Counter { count: 0 });
        timeout(first_a.loaded()).await;
        timeout(second_a.loaded()).await;

        let client_b = BlockClient::new(Uuid::new_v4());
        client_b.connect(url);
        let first_b = client_b.get_block::<Counter>(first_a.id());
        let second_b = client_b.get_block::<Counter>(second_a.id());
        timeout(first_b.loaded()).await;
        timeout(second_b.loaded()).await;

        let observed_second = second_b.clone();
        let observation = tokio::spawn(async move {
            first_b
                .wait_until(|counter| {
                    counter.count == 1 && observed_second.read().unwrap().count == 2
                })
                .await;
        });

        client_a.batch(|batch| {
            batch.operate(&first_a, CounterOperation::Add(1));
            batch.operate(&second_a, CounterOperation::Add(2));
        });
        timeout(client_a.synchronized()).await;
        tokio::time::timeout(Duration::from_secs(2), observation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second_b.read().unwrap().count, 2);

        drop(first_a);
        drop(second_a);
        drop(second_b);
        drop(client_a);
        drop(client_b);
        server.abort();
        let _ = server.await;
        fs::remove_dir_all(data_dir).await.unwrap();
    }

    #[tokio::test]
    async fn crdt_text_clients_converge_after_concurrent_insertions() {
        let data_dir = env::temp_dir().join(format!("block-e2e-text-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_data_dir = data_dir.clone();
        let server = tokio::spawn(async move {
            block_server::serve(listener, server_data_dir)
                .await
                .unwrap();
        });
        let url = format!("ws://{address}");

        let client_a = BlockClient::new(Uuid::new_v4());
        client_a.connect(url.clone());
        let block_a = client_a.create_block(TextDocument::new());
        timeout(block_a.loaded()).await;

        let client_b = BlockClient::new(Uuid::new_v4());
        client_b.connect(url);
        let block_b = client_b.get_block::<TextDocument>(block_a.id());
        timeout(block_b.loaded()).await;

        let operation = {
            let document = block_a.read().unwrap();
            document.insert_operation(0, 'a').unwrap()
        };
        block_a.operate(operation);
        let operation = {
            let document = block_a.read().unwrap();
            document.insert_operation(1, 'b').unwrap()
        };
        block_a.operate(operation);
        let operation = {
            let document = block_b.read().unwrap();
            document.insert_operation(0, 'x').unwrap()
        };
        block_b.operate(operation);
        let operation = {
            let document = block_b.read().unwrap();
            document.insert_operation(1, 'y').unwrap()
        };
        block_b.operate(operation);
        timeout(client_a.synchronized()).await;
        timeout(client_b.synchronized()).await;
        timeout(block_a.wait_until(|document| document.len() == 4)).await;
        timeout(block_b.wait_until(|document| document.len() == 4)).await;

        let text_a = block_a.read().unwrap().text();
        let text_b = block_b.read().unwrap().text();
        assert_eq!(text_a, text_b);
        assert_eq!(text_a.chars().count(), 4);

        drop(block_a);
        drop(block_b);
        drop(client_a);
        drop(client_b);
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
