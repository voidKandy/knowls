use super::{
    init_clientside_listener_and_stream, SocketMessage, CLIENTSIDE_CLI_ADDR, SERVERSIDE_CLI_ADDR,
};
use crate::{
    config::GLOBAL_SYS_CONFIG,
    state::SharedState,
    ui::cli::{CliRequest, CliResponse},
};
use std::sync::LazyLock;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

impl SocketMessage for CliResponse {}
impl SocketMessage for CliRequest {}

pub async fn send_request(request: CliRequest) -> UnixListener {
    if !LazyLock::force(&GLOBAL_SYS_CONFIG).exists() {
        panic!("It looks like you haven't set up configuration for espx. Create a valid config file at $HOME/.espx/config.toml");
    }

    let (unix_listener, mut server_connection) =
        init_clientside_listener_and_stream(SERVERSIDE_CLI_ADDR, CLIENTSIDE_CLI_ADDR).await;

    let bytes = request
        .as_bytes_to_send()
        .expect("failed to get bytes from command");
    server_connection
        .write_all(&bytes)
        .await
        .expect("Failed to write to server");
    server_connection.flush().await.unwrap();
    warn!("sent message to server");
    unix_listener
}

pub async fn wait_for_response(listener: UnixListener) -> CliResponse {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let mut buf_reader = BufReader::new(stream);
                let mut buf = String::new();
                loop {
                    let bytes = buf_reader.read_line(&mut buf).await.unwrap();
                    if bytes == 0 {
                        warn!("Closed");
                        panic!("recieved an empty message from the server");
                    }

                    if let Some(msg) = serde_json::from_str::<CliResponse>(&buf).ok() {
                        warn!("Rcv: {msg:#?}");
                        return msg;
                    }
                    buf.clear();
                }
            }
            Err(err) => {
                warn!("err connecting: {err:#?}");
            }
        }
    }
}

pub async fn handle_cli_req(
    stream: UnixStream,
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
                    msg.handle(state, stream).await;
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
