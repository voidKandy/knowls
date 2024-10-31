use crate::{
    handle::{self, buffer_operations::BufferOpChannelStatus},
    state::SharedState,
};
use anyhow::anyhow;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::RwLock,
};
use tracing::warn;

const TUI_SOCKET_ADDR: &str = "/tmp/espx_tui_socket.sock";
const LSP_SOCKET_ADDR: &str = "/tmp/espx_lsp_socket.sock";
pub async fn init_socket_listener_and_stream() -> (UnixListener, UnixStream) {
    if Path::new(TUI_SOCKET_ADDR).exists() {
        std::fs::remove_file(TUI_SOCKET_ADDR).unwrap();
    }

    let unix_listener = UnixListener::bind(TUI_SOCKET_ADDR).unwrap();
    warn!("created socket at: {TUI_SOCKET_ADDR}");

    #[allow(unused_assignments)]
    let mut unix_stream_opt = Option::<UnixStream>::None;

    loop {
        match UnixStream::connect(LSP_SOCKET_ADDR).await {
            Ok(stream) => {
                warn!("connected to lsp socket");
                unix_stream_opt = Some(stream);
                break;
            }

            Err(_) => {
                warn!("did not connect to socket at {LSP_SOCKET_ADDR}\nsleeping")
            }
        }

        tokio::time::sleep(Duration::from_millis(2000)).await;
    }

    (
        unix_listener,
        unix_stream_opt.expect("Must have exited loop early"),
    )
}

pub async fn unix_socket_loop(
    shared_stream: Arc<RwLock<UnixStream>>,
    listener: UnixListener,
    mut state: SharedState,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                warn!("client: {addr:?}");
                let unix_stream = Arc::clone(&shared_stream);
                state.get_write().unwrap().attached = Some(addr);
                let mut state = state.clone();

                tokio::spawn(async move {
                    let mut buf_reader = BufReader::new(stream);
                    let mut buf = String::new();
                    loop {
                        let bytes = buf_reader.read_line(&mut buf).await.unwrap();
                        if bytes == 0 {
                            warn!("Closed");
                            break;
                        }

                        if let Some(msg) = serde_json::from_str::<lsp_server::Message>(&buf).ok() {
                            warn!("Rcv: {msg:#?}");
                            match match msg {
                                lsp_server::Message::Notification(not) => {
                                    handle::notifications::handle_notification(not, state.clone())
                                        .await
                                }
                                lsp_server::Message::Request(req) => {
                                    handle::requests::handle_request(req, state.clone()).await
                                }
                                _ => Err(anyhow!("No handler for responses").into()),
                            } {
                                Ok(mut buffer_op_channel_handler) => {
                                    while let Some(status) =
                                        buffer_op_channel_handler.receiver.recv().await
                                    {
                                        match status.unwrap() {
                                            BufferOpChannelStatus::Finished => break,
                                            BufferOpChannelStatus::Working(buffer_op) => {
                                                buffer_op
                                                    .do_operation(Arc::clone(&unix_stream))
                                                    .await
                                                    .unwrap();
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("error in handler: {}", err);
                                }
                            }
                        }
                        buf.clear();
                    }
                    state.get_write().unwrap().attached = None;
                });
            }
            Err(err) => {
                warn!("error connecting {err:#?}")
            }
        }
    }
}
