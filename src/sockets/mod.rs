// pub mod client;
pub mod rpc;
// pub mod server;
pub mod trace;
use crate::other_err;

use std::{fmt::Debug, path::Path, time::Duration};
use tokio::net::{UnixListener, UnixStream};
use tracing::warn;

pub const SERVERSIDE_RELAY_ADDR: &'static str = "/tmp/relay_srv.sock";
pub const CLIENTSIDE_RELAY_ADDR: &'static str = "/tmp/relay_clnt.sock";
pub const SERVERSIDE_CLI_ADDR: &'static str = "/tmp/cli_srv.sock";
pub const CLIENTSIDE_CLI_ADDR: &'static str = "/tmp/cli_clnt.sock";

#[derive(Debug)]
pub struct SocketGuard<'a> {
    path: &'a Path,
}

impl<'a> Drop for SocketGuard<'a> {
    fn drop(&mut self) {
        warn!("dropping socket: {:#?}", self.path);
        std::fs::remove_file(self.path).unwrap();
    }
}

impl<'a> From<&'a str> for SocketGuard<'a> {
    fn from(p: &'a str) -> Self {
        let path = Path::new(p);

        if path.exists() {
            std::fs::remove_file(p).unwrap();
        }
        let _ = std::fs::File::create(path)
            .map_err(|err| warn!("failed to create socket file: {err:#?}"))
            .unwrap();
        warn!("created socket at: {path:#?}");
        Self { path }
    }
}

// pub trait SocketMessage: Serialize + Clone + Debug + for<'d> Deserialize<'d> {
//     fn as_bytes_to_send(&self) -> serde_json::Result<Vec<u8>> {
//         let value: serde_json::Value = serde_json::to_value(self)?;
//
//         let str = serde_json::to_string(&value)?;
//         let str = &format!("{str}\n");
//         Ok(str.as_bytes().to_vec())
//     }
// }

pub(super) async fn init_clientside_listener_and_stream<'a>(
    server_socket: &SocketGuard<'a>,
    client_socket: &SocketGuard<'a>,
) -> (UnixListener, UnixStream) {
    let listener = UnixListener::bind(client_socket.path).unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let server_connection = UnixStream::connect(server_socket.path)
        .await
        .expect("failed to open write stream to server");
    (listener, server_connection)
}

/// returns Err only if the connection fails
pub async fn init_serverside_listener_and_stream<'a>(
    server_socket_path: impl Into<SocketGuard<'a>>,
    client_socket_path: impl Into<SocketGuard<'a>>,
) -> crate::MainResult<(UnixListener, UnixStream)> {
    let server_socket: SocketGuard = server_socket_path.into();
    let client_socket: SocketGuard = client_socket_path.into();
    let unix_listener = UnixListener::bind(server_socket.path).unwrap();

    let unix_stream = UnixStream::connect(client_socket.path)
        .await
        .map_err(|err| {
            other_err!(
                "did not connect to socket at {:#?}\n: {err:#?}",
                server_socket.path
            )
        })?;
    Ok((unix_listener, unix_stream))
}
