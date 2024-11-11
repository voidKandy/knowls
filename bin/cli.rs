use clap::Parser;
use espx_lsp_server::{
    sockets::{send_request, wait_for_response, CLIENTSIDE_CLI_ADDR, CLI_TRACING},
    ui::cli::CliArgs,
};
use std::sync::LazyLock;
use tracing::warn;
#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    let args = CliArgs::parse();
    warn!("{args:?}");

    if let Some(req) = args.command.handle() {
        let listener = send_request(req).await;
        let cli_response = wait_for_response(listener).await;
        cli_response.handle();
        std::fs::remove_file(CLIENTSIDE_CLI_ADDR).unwrap();
    }
}
