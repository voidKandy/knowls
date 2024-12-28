use espx_lsp_server::{
    sockets::{start_lsp_relay, RELAY_TRACING},
    MainResult,
};
use std::sync::LazyLock;

#[tokio::main]
async fn main() -> MainResult<()> {
    LazyLock::force(&RELAY_TRACING);
    start_lsp_relay().await
}
