use crate::{
    agents::{AgentID, Agents},
    config::{
        espx::{ModelConfig, ModelProvider},
        Config, GLOBAL_SYS_CONFIG,
    },
    sockets::{SocketMessage, CLI_TRACING_LOG_FILE},
    state::SharedState,
};
use clap::{Parser, Subcommand};
use lsp_types::Uri;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, path::PathBuf, str::FromStr, sync::LazyLock};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

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
    Ping,
    Report,
    Prompt {
        #[arg(short = 'a', long)]
        agent_name: String,
        #[arg(short = 'p', long)]
        prompt: String,
    },
}

impl TryFrom<CliCommand> for CliRequest {
    type Error = anyhow::Error;
    fn try_from(value: CliCommand) -> Result<Self, Self::Error> {
        match value {
            CliCommand::Prompt { agent_name, prompt } => Ok(Self::Prompt { agent_name, prompt }),
            CliCommand::Ping => Ok(Self::Ping),
            CliCommand::Report => Ok(Self::Report),
            _ => Err(anyhow::anyhow!(
                "{value:#?} cannot be turned into a cli request"
            )),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum CliRequest {
    Ping,
    Report,
    Prompt { agent_name: String, prompt: String },
}

#[allow(private_interfaces)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CliResponse {
    Ping,
    Report(String),
    AgentResponse(AgentPromptResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum AgentPromptResponse {
    Ok(String),
    Err(String),
}

impl Into<String> for &AgentPromptResponse {
    fn into(self) -> String {
        match &self {
            &AgentPromptResponse::Ok(s) => s,
            &AgentPromptResponse::Err(s) => s,
        }
        .to_string()
    }
}

impl CliResponse {
    pub fn handle(&self) {
        match self {
            CliResponse::AgentResponse(res) => {
                println!("{}", Into::<String>::into(res));
            }
            CliResponse::Ping => {
                println!("received a ping");
            }
            CliResponse::Report(report_str) => {
                println!(
                    "\n____________________________\n{report_str}\n____________________________"
                );
            }
        }
    }
}

impl CliCommand {
    /// Either handles the command or passes it along to be sent to the server
    pub fn handle(self) -> Option<CliRequest> {
        match self {
            Self::Logs { clear } => {
                let path = PathBuf::from_str(CLI_TRACING_LOG_FILE).unwrap();
                let log_file_content = fs::read_to_string(&path).unwrap();
                if clear {
                    fs::write(path, b"").unwrap();
                }
                println!("{log_file_content}");
                None
            }
            other => Some(CliRequest::try_from(other).expect("couldn't turn command into req")),
        }
    }
}

impl CliRequest {
    pub async fn handle(&self, state: SharedState<'static>, mut stream: UnixStream) {
        match self {
            CliRequest::Prompt { agent_name, prompt } => {
                let mut w = state.0.try_write().expect("could not write lock");
                let mut prompt_response = Option::<AgentPromptResponse>::None;

                let invalid_agent_name_err = |agents: &Agents| -> String {
                    let mut buffer = String::new();
                    buffer.push_str(&format!("{agent_name} is not a valid agent name\n"));
                    buffer.push_str("Valid Agent Names:\n");
                    for (id, _) in agents.iter_agents() {
                        buffer.push_str(id.to_string().as_str());
                    }
                    buffer
                };

                if let Some(agents) = w.agents.as_mut() {
                    if let Some(agent) = match agent_name.trim().chars().count().cmp(&1) {
                        std::cmp::Ordering::Less => {
                            prompt_response =
                                Some(AgentPromptResponse::Err(invalid_agent_name_err(agents)));
                            None
                        }

                        std::cmp::Ordering::Equal => {
                            let char = agent_name.trim().chars().next().unwrap();
                            warn!("agent id char: {char}");
                            AgentID::try_from_char(char).and_then(|id| agents.get_agent_mut(id))
                        }

                        std::cmp::Ordering::Greater => match Uri::from_str(&agent_name) {
                            Ok(uri) => agents.get_agent_mut(&uri),
                            Err(_) => {
                                prompt_response =
                                    Some(AgentPromptResponse::Err(invalid_agent_name_err(agents)));
                                None
                            }
                        },
                    } {
                        warn!("got agent, prompting....");
                        agent
                            .cache
                            .push(espionox::prelude::Message::new_user(&prompt));
                        let a_response = agent
                            .io_completion()
                            .await
                            .expect("failed to get agent io completion");
                        prompt_response = Some(AgentPromptResponse::Ok(a_response));
                    }

                    let response = prompt_response.unwrap_or(AgentPromptResponse::Err(
                        String::from("Somehow agent prompt response was empty"),
                    ));
                    let bytes = CliResponse::AgentResponse(response)
                        .as_bytes_to_send()
                        .unwrap();
                    stream
                        .write_all(&bytes)
                        .await
                        .expect("failed to send bytes");
                    stream.flush().await.unwrap();
                }
            }

            CliRequest::Report => {
                let r = state.0.try_read().expect("could not read lock");
                let mut report_str = String::new();
                if let Some(agents) = &r.agents {
                    let amt = agents.iter_agents().count();
                    report_str.push_str(&format!("{amt} Agents\n"));
                    for (id, agent) in agents.iter_agents() {
                        report_str.push_str(&format!("{id:#?} - {} Messages\n", agent.cache.len()));
                    }
                }

                if let Some(db) = &r.database {
                    match db.thread {
                        Some(_) => report_str.push_str(&format!(
                            "Database is running\nAddress - {:#?}\nConfig - {:#?}\n",
                            db.path, db.config
                        )),
                        None => report_str.push_str("No Database\n"),
                    }
                }
                report_str.push_str(&format!("{} Documents", r.documents.len()));
                let bytes = CliResponse::Report(report_str).as_bytes_to_send().unwrap();
                stream
                    .write_all(&bytes)
                    .await
                    .expect("failed to send bytes");
                stream.flush().await.unwrap();
            }

            CliRequest::Ping => {
                let response = CliResponse::Ping;
                let bytes = response
                    .as_bytes_to_send()
                    .expect("failed to get bytes for cli response");
                stream
                    .write_all(&bytes)
                    .await
                    .expect("failed to send bytes");
                stream.flush().await.unwrap();
            }
        }
    }
}
