use super::{
    init_clientside_listener_and_stream, SocketMessage, CLIENTSIDE_CLI_ADDR, SERVERSIDE_CLI_ADDR,
};
use crate::{
    agents::{AgentID, Agents},
    sockets::trace::CLI_TRACING_LOG_FILE,
    state::SharedState,
};
use clap::{Parser, Subcommand};
use lsp_types::{OneOf, Uri};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, path::PathBuf, str::FromStr};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
    #[command(subcommand, name = "cli request")]
    pub command: CliRequest,
}

#[derive(Debug, Subcommand, Clone, Deserialize, Serialize)]
pub enum CliRequest {
    Ping,
    Report,
    Logs {
        #[arg(short = 'c', long)]
        clear: bool,
    },
    Prompt {
        #[arg(short = 'a', long)]
        agent_name: String,
        #[arg(short = 'p', long)]
        prompt: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CliResponse {
    Ping,
    Report(String),
    AgentResponse(AgentPromptResponse),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum AgentPromptResponse {
    Ok(String),
    Err(String),
}

impl Into<String> for AgentPromptResponse {
    fn into(self) -> String {
        match self {
            Self::Ok(s) => s,
            Self::Err(s) => s,
        }
    }
}

impl SocketMessage for CliResponse {}
impl SocketMessage for CliRequest {}

pub async fn run_command(command: CliRequest) {
    if let CliRequest::Logs { clear } = command {
        let path = PathBuf::from_str(CLI_TRACING_LOG_FILE).unwrap();
        let log_file_content = fs::read_to_string(&path).unwrap();
        if clear {
            fs::write(path, b"").unwrap();
        }
        println!("{log_file_content}");
        return;
    }

    let (unix_listener, mut server_connection) =
        init_clientside_listener_and_stream(SERVERSIDE_CLI_ADDR, CLIENTSIDE_CLI_ADDR).await;

    let bytes = command
        .as_bytes_to_send()
        .expect("failed to get bytes from command");
    server_connection.write_all(&bytes).await.unwrap();
    server_connection.flush().await.unwrap();
    warn!("sent message to server");

    match unix_listener.accept().await {
        Ok((stream, _addr)) => {
            let mut buf_reader = BufReader::new(stream);
            let mut buf = String::new();
            loop {
                let bytes = buf_reader.read_line(&mut buf).await.unwrap();
                if bytes == 0 {
                    warn!("Closed");
                    break;
                }

                if let Some(msg) = serde_json::from_str::<CliResponse>(&buf).ok() {
                    warn!("Rcv: {msg:#?}");
                    match msg {
                        CliResponse::AgentResponse(res) => {
                            println!("{}", Into::<String>::into(res));
                        }
                        CliResponse::Ping => {
                            println!("received a ping");
                        }
                        CliResponse::Report(report_str) => {
                            println!("\n____________________________\n{report_str}\n____________________________");
                        }
                    }
                    break;
                }
                buf.clear();
            }
        }
        Err(err) => {
            warn!("err connecting: {err:#?}");
        }
    }
    std::fs::remove_file(CLIENTSIDE_CLI_ADDR).unwrap();
}

pub async fn handle_cli_req(
    mut stream: UnixStream,
    listener: UnixListener,
    state: SharedState<'static>,
) {
    loop {
        match listener.accept().await {
            Ok((server_connection, addr)) => {
                warn!("cli connected: {addr:?}");
                let mut buf_reader = BufReader::new(server_connection);
                let mut buf = String::new();

                let bytes = buf_reader.read_line(&mut buf).await.unwrap();
                if bytes == 0 {
                    warn!("Closed");
                    return;
                }

                if let Some(msg) = serde_json::from_str::<CliRequest>(&buf).ok() {
                    warn!("Rcv: {msg:#?}");
                    match msg {
                        CliRequest::Logs { .. } => {
                            unreachable!("logs request should never be sent")
                        }

                        CliRequest::Prompt { agent_name, prompt } => {
                            let mut w = state.0.try_write().expect("could not write lock");
                            let mut prompt_response = Option::<AgentPromptResponse>::None;

                            let invalid_agent_name_err = |agents: &Agents| -> String {
                                let mut buffer = String::new();
                                buffer
                                    .push_str(&format!("{agent_name} is not a valid agent name\n"));
                                buffer.push_str("Valid Agent Names:\n");
                                for (id, _) in agents.iter_agents() {
                                    buffer.push_str(id.to_string().as_str());
                                }
                                buffer
                            };

                            if let Some(agents) = w.agents.as_mut() {
                                if let Some(agent) = match agent_name.trim().chars().count().cmp(&1)
                                {
                                    std::cmp::Ordering::Less => {
                                        prompt_response = Some(AgentPromptResponse::Err(
                                            invalid_agent_name_err(agents),
                                        ));
                                        None
                                    }

                                    std::cmp::Ordering::Equal => {
                                        let char = agent_name.trim().chars().next().unwrap();
                                        warn!("agent id char: {char}");
                                        AgentID::try_from_char(char)
                                            .and_then(|id| agents.get_agent_mut(id))
                                    }

                                    std::cmp::Ordering::Greater => {
                                        match Uri::from_str(&agent_name) {
                                            Ok(uri) => agents.get_agent_mut(&uri),
                                            Err(_) => {
                                                prompt_response = Some(AgentPromptResponse::Err(
                                                    invalid_agent_name_err(agents),
                                                ));
                                                None
                                            }
                                        }
                                    }
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
                                    report_str.push_str(&format!(
                                        "{id:#?} - {} Messages\n",
                                        agent.cache.len()
                                    ));
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
                return;
            }
            Err(err) => {
                warn!("error connecting {err:#?}")
            }
        }
    }
}

mod tests {
    use std::{sync::LazyLock, time::Duration};

    use tokio::{io::AsyncWriteExt, time::Timeout};

    use crate::sockets::{
        init_clientside_listener_and_stream, init_serverside_listener_and_stream,
        CLIENTSIDE_CLI_ADDR, SERVERSIDE_CLI_ADDR, SERVERSIDE_RELAY_ADDR,
    };

    #[tokio::test]
    async fn cli_socket_healthy() {
        let (serverside_listener, _) = tokio::time::timeout(Duration::from_millis(5000), {
            init_serverside_listener_and_stream(SERVERSIDE_CLI_ADDR, CLIENTSIDE_CLI_ADDR)
        })
        .await
        .expect("server connection timeout");
        println!("created serverside listener");

        let (clientside_listener, mut server_connection) =
            tokio::time::timeout(Duration::from_millis(5000), {
                init_clientside_listener_and_stream(SERVERSIDE_CLI_ADDR, CLIENTSIDE_CLI_ADDR)
            })
            .await
            .expect("client connection timeout");
        println!("created clientside listener");

        server_connection.write_all(b"health check").await.unwrap();
        println!("said hello from client");

        let _ = serverside_listener.accept().await.unwrap();
    }
}
