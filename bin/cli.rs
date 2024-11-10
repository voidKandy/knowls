use clap::Parser;
use espx_lsp_server::{
    sockets::{run_command, CliRequest, CLI_TRACING},
    telemetry::TRACING,
};
use std::sync::LazyLock;
use tracing::warn;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(value_name = "command")]
    command: CliRequest,
}

#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    let args = Args::parse();
    warn!("{args:?}");
    run_command(args.command).await;
}
