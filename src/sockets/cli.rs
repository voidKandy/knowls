use super::{
    init_clientside_listener_and_stream, SocketMessage, CLIENTSIDE_CLI_ADDR, SERVERSIDE_CLI_ADDR,
};
use crate::{sockets::trace::CLI_TRACING_LOG_FILE, state::SharedState};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, path::PathBuf, str::FromStr};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

#[derive(Debug, ValueEnum, Clone, Deserialize, Serialize)]
pub enum CliRequest {
    Ping,
    Logs,
}

#[derive(Debug, ValueEnum, Clone, Deserialize, Serialize)]
pub enum CliResponse {
    Ping,
}

impl SocketMessage for CliResponse {}
impl SocketMessage for CliRequest {}

pub async fn run_command(command: CliRequest) {
    if let CliRequest::Logs = command {
        let log_file_content =
            fs::read_to_string(PathBuf::from_str(CLI_TRACING_LOG_FILE).unwrap()).unwrap();
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
                        CliResponse::Ping => {
                            println!("received a ping");
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
}

pub async fn from_cli_recv_loop(
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
                    break;
                }

                if let Some(msg) = serde_json::from_str::<CliRequest>(&buf).ok() {
                    warn!("Rcv: {msg:#?}");
                    match msg {
                        CliRequest::Logs => unreachable!("logs request should never be sent"),
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
