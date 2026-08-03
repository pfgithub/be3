use std::{env, path::PathBuf};

use tokio::net::TcpListener;

const DEFAULT_ADDR: &str = "127.0.0.1:9090";
const DEFAULT_DATA_DIR: &str = "block-data";

#[tokio::main]
async fn main() -> Result<(), block_server::ServerError> {
    let mut args = env::args().skip(1);
    let address = args.next().unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let data_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
    let listener = TcpListener::bind(&address).await?;
    let database_path = data_dir.join("blocks.sqlite3");

    println!(
        "block server listening on http://{} storing blocks in {}",
        address,
        database_path.display()
    );
    block_server::serve(listener, data_dir).await
}
