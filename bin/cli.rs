use clap::Parser;
use espx_lsp_server::{
    sockets::{run_command, CliArgs, CliRequest, CLI_TRACING},
    telemetry::TRACING,
};
use std::sync::LazyLock;
use tracing::warn;

#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    let args = CliArgs::parse();
    warn!("{args:?}");
    run_command(args.command).await;
}
