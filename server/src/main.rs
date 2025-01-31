mod server;
mod trace;
use knowls::{trace_panics, MainResult};
use server::Server;
use std::sync::LazyLock;

#[tokio::main]
async fn main() {
    LazyLock::force(&trace::TRACING);
    tracing::warn!("spinning up LSP Server");
    trace_panics!();
    let addr = "127.0.0.1:1598";
    let server = Server::init(addr).expect("failed to init server");
    server.main_loop().await.unwrap();
}
