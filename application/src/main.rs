mod database;
mod lsp;
mod rpc_handler;
mod state;
mod trace;
mod tui;

use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicBool, Arc},
};

use clap::Parser;
use database::{config::DatabaseConfig, models::Knowledge, Database, Record};
use knowls::MainResult;
use rpc_handler::RpcConnectionHandler;
use state::State;
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle};
use tui::config::Config;

#[tokio::main]
async fn main() {
    // LazyLock::force(&trace::TRACING);

    tui::errors::init().unwrap();
    tui::logging::init().unwrap();

    let args = tui::cli::Cli::parse();
    let config = Config::new().expect("failed to get config");

    let database = Database::new(DatabaseConfig::default()).await.unwrap();
    let state = mock_state(database).await;
    let shared_state = Arc::new(RwLock::new(state));

    let tcp_listener = TcpListener::bind(args.rpc_addr)
        .await
        .expect("failed to bind listener");
    let mut connection_handler = RpcConnectionHandler::new(
        config.completion_config.clone(),
        tcp_listener,
        Arc::clone(&shared_state),
    );

    let should_run = Arc::new(AtomicBool::new(true));
    let handler_should_run = Arc::clone(&should_run);
    let join_handle = tokio::spawn(async move {
        connection_handler
            .main_loop(handler_should_run)
            .await
            .unwrap();
    });

    let mut app = tui::App::new(config, shared_state, args.tick_rate, args.frame_rate)
        .await
        .unwrap();
    app.run().await.unwrap();

    should_run.store(false, std::sync::atomic::Ordering::Relaxed);
    join_handle.await.unwrap();
}

async fn mock_state(database: Database) -> State {
    let mut knowledge = HashMap::new();

    let mock_entries = vec![
        (
            PathBuf::from_str("testknowledge/1").unwrap(),
            "Zig is a general-purpose programming language.",
        ),
        (
            PathBuf::from_str("testknowledge/2").unwrap(),
            "Rust provides memory safety without garbage collection.",
        ),
        (
            PathBuf::from_str("testknowledge/3").unwrap(),
            "SurrealDB is a multi-model database for web applications.",
        ),
    ];

    for (path, content) in mock_entries {
        let k = Knowledge::new(path, content);
        let r: Option<Record<Knowledge>> = database
            .client
            .create("knowledge")
            .content(k)
            .await
            .unwrap();
        let r = r.unwrap();
        knowledge.insert(r.id, r.obj);
    }

    State {
        database,
        knowledge,
        // lsp_documents: HashMap::new(),
        connections: HashMap::new(),
    }
}
