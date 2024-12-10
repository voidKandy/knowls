mod relay;
mod trace;
pub use self::{
    // cli::{handle_cli_req, send_request, wait_for_response},
    relay::{from_relay_recv_loop, start_lsp_relay},
    trace::{CLI_TRACING, CLI_TRACING_LOG_FILE, RELAY_TRACING},
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::Path, time::Duration};
use tokio::net::{UnixListener, UnixStream};
use tracing::warn;

pub const SERVERSIDE_RELAY_ADDR: &str = "/tmp/relay_srv.sock";
pub const CLIENTSIDE_RELAY_ADDR: &str = "/tmp/relay_clnt.sock";

pub const SERVERSIDE_CLI_ADDR: &str = "/tmp/cli_srv.sock";
pub const CLIENTSIDE_CLI_ADDR: &str = "/tmp/cli_clnt.sock";

pub trait SocketMessage: Serialize + Clone + Debug + for<'d> Deserialize<'d> {
    fn as_bytes_to_send(&self) -> serde_json::Result<Vec<u8>> {
        let value: serde_json::Value = serde_json::to_value(self)?;

        let str = serde_json::to_string(&value)?;
        let str = &format!("{str}\n");
        Ok(str.as_bytes().to_vec())
    }
}

pub(super) async fn init_clientside_listener_and_stream(
    server_addr: &str,
    client_addr: &str,
) -> (UnixListener, UnixStream) {
    let path = Path::new(client_addr);
    if path.exists() {
        std::fs::remove_file(client_addr).unwrap();
    }

    let listener = UnixListener::bind(client_addr).unwrap();
    warn!("created socket at: {client_addr}");
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let server_connection = UnixStream::connect(server_addr)
        .await
        .expect("failed to open write stream to server");
    (listener, server_connection)
}

pub async fn init_serverside_listener_and_stream(
    server_addr: &str,
    client_addr: &str,
) -> (UnixListener, UnixStream) {
    let path = Path::new(server_addr);
    if path.exists() {
        std::fs::remove_file(server_addr).unwrap();
    }

    let unix_listener = UnixListener::bind(server_addr).unwrap();
    warn!("created socket at: {server_addr}");

    #[allow(unused_assignments)]
    let mut unix_stream_opt = Option::<UnixStream>::None;

    loop {
        match UnixStream::connect(client_addr).await {
            Ok(stream) => {
                warn!("connected to lsp socket");
                unix_stream_opt = Some(stream);
                break;
            }

            Err(_) => {
                warn!("did not connect to socket at {server_addr}\nsleeping")
            }
        }

        tokio::time::sleep(Duration::from_millis(2000)).await;
    }

    (
        unix_listener,
        unix_stream_opt.expect("Must have exited loop early"),
    )
}
