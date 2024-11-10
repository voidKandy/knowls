#![allow(unused)]
use clap::Parser;
use espx_lsp_server::{self, config::Config, state::SharedState, telemetry::TRACING};

use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'c', long)]
    config_file: String,
}

#[cfg(feature = "gui")]
use espx_lsp_server::ui::run_gui;

#[tokio::main]
#[cfg(feature = "gui")]
async fn main() -> eframe::Result<()> {
    use espx_lsp_server::sockets::{
        from_cli_recv_loop, from_relay_recv_loop, init_serverside_listener_and_stream,
        CLIENTSIDE_CLI_ADDR, CLIENTSIDE_RELAY_ADDR, SERVERSIDE_CLI_ADDR, SERVERSIDE_RELAY_ADDR,
    };

    LazyLock::force(&TRACING);
    let args = Args::parse();
    println!("{args:?}");
    let config = Config::init_from_file_path(&args.config_file)
        .expect("could not get config from given path");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).unwrap();

    // LSP relay connection
    let lsp_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        from_relay_recv_loop(unix_stream, unix_listener, lsp_thread_state).await
    });

    // CLI relay connection
    let cli_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_CLI_ADDR, CLIENTSIDE_CLI_ADDR).await;
        from_cli_recv_loop(unix_stream, unix_listener, cli_thread_state).await
    });

    run_gui(state)
}

#[cfg(not(feature = "gui"))]
fn main() {
    println!(
        r#"
    
    +--------------------------WARNING---------------------------+
    | You can't run this binary without enabling the gui feature |
    |       Try adding --features "gui" after --bin gui          |
    +------------------------------------------------------------+

"#
    );
    std::process::exit(1)
}
