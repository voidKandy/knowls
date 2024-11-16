pub mod new_tui;
// pub mod user_input;
use crate::{agents::AgentID, sockets::CLI_TRACING_LOG_FILE, state::SharedState};
use clap::{Parser, Subcommand};
use lsp_types::Uri;
use new_tui::Tui;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, str::FromStr, sync::LazyLock};
use tracing::warn;
// use user_input::ChatApp;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand, name = "cli command")]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum CliCommand {
    Logs {
        #[arg(short = 'c', long)]
        clear: bool,
    },
    Start {
        #[arg(short = 'a', long)]
        agent_name: String,
    },
}

pub fn string_to_agent_id(agent_name: &str) -> Option<AgentID> {
    match agent_name.trim().chars().count().cmp(&1) {
        std::cmp::Ordering::Less => None,

        std::cmp::Ordering::Equal => {
            let char = agent_name.trim().chars().next().unwrap();
            warn!("agent id char: {char}");
            AgentID::try_from_char(char)
        }

        std::cmp::Ordering::Greater => match Uri::from_str(&agent_name) {
            Ok(_) => Some(AgentID::Uri(agent_name.to_string())),
            Err(_) => None,
        },
    }
}

impl CliCommand {
    #[tracing::instrument(name = "cli command handler", skip(state))]
    pub fn handle(self, state: SharedState<'static>) -> Option<Tui> {
        match self {
            Self::Logs { clear } => {
                let log_file_content =
                    fs::read_to_string(LazyLock::force(&CLI_TRACING_LOG_FILE)).unwrap();
                if clear {
                    fs::write(LazyLock::force(&CLI_TRACING_LOG_FILE), b"").unwrap();
                }
                println!("{log_file_content}");
                None
            }
            Self::Start { agent_name } => {
                let agent_id = string_to_agent_id(&agent_name).unwrap();
                let app = Tui::new(state, agent_id);
                Some(app)
            }
        }
    }
}
