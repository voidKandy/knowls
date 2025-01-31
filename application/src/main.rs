mod app;
mod database;
mod trace;
mod tui;

use app::Application;
use clap::Parser;
use database::{config::DatabaseConfig, Database};
use std::{
    ops::RangeToInclusive,
    sync::{Arc, LazyLock},
};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    // LazyLock::force(&trace::TRACING);

    tui::errors::init().unwrap();
    tui::logging::init().unwrap();

    let args = tui::cli::Cli::parse();
    let mut app = tui::App::new(args.tick_rate, args.frame_rate).unwrap();
    app.run().await.unwrap();
    // color_eyre::install().expect("failed to prepare color_eyre");
    // let terminal = ratatui::init();
    // let database = Database::new(DatabaseConfig::default())
    //     .await
    //     .expect("failed to create database");
    // let mut app = Application::new(database, "127.0.0.1:8778")
    //     .await
    //     .expect("failed to create app");
    // let tui = app::tui::Tui::new(app.clone_state()).await;

    // tokio::spawn(async move { app.main_loop().await });

    // tui.run(terminal).expect("terminal failed");
    // ratatui::restore();
}
