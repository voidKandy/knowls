pub mod action;
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod errors;
pub mod logging;
pub mod tui;

pub use app::App;

// #[tokio::main]
// async fn main() -> Result<()> {
//     crate::errors::init()?;
//     crate::logging::init()?;

//     let args = Cli::parse();
//     let mut app = App::new(args.tick_rate, args.frame_rate)?;
//     app.run().await?;
//     Ok(())
// }
