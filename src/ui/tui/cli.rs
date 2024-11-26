use crate::{agents::AgentID, sockets::CLI_TRACING_LOG_FILE, state::SharedState};
use clap::{Parser, Subcommand};
use lsp_types::Uri;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, str::FromStr, sync::LazyLock};
use tracing::warn;

use super::Tui;

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
    Start,
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
