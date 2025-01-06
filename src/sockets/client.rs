use crate::{other_err, sockets::SocketGuard, MainResult};
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Interest},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

pub struct RelayClient {
    /// Listener for own socket
    socket_listener: UnixListener,
    /// Streams to external sockets
    socket_stream: UnixStream,
    lsp_connection: lsp_server::Connection,
    init_params: InitializeParams,
}

impl RelayClient {
    /// assumes a server is already running, will return Err if one is not
    #[tracing::instrument(name = "new relay client")]
    pub async fn new(
        server_socket_guard: &SocketGuard<'static>,
        client_socket_guard: &SocketGuard<'static>,
    ) -> MainResult<Self> {
        let socket_stream = UnixStream::connect(socket_guard.path).await?;

        let (lsp_connection, _io_threads) = lsp_server::Connection::stdio();

        let text_document_sync = Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(true),
                    },
                )),
                change: Some(TextDocumentSyncKind::FULL),

                ..Default::default()
            },
        ));

        let server_capabilities = serde_json::to_value(ServerCapabilities {
            text_document_sync,
            completion_provider: Some(lsp_types::CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(vec!["?".to_string(), "\"".to_string(), " ".to_string()]),
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
                all_commit_characters: None,
                completion_item: None,
            }),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                lsp_types::DiagnosticOptions::default(),
            )),
            definition_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        })
        .unwrap();

        let params = lsp_connection
            .initialize(server_capabilities)
            .expect("failed to initialize");
        let init_params: InitializeParams = serde_json::from_value(params).unwrap();

        Ok(Self {
            socket_stream,
            lsp_connection,
            init_params,
        })
    }

    #[tracing::instrument(name = "relay client main loop", skip_all)]
    pub async fn main_loop(self) -> MainResult<()> {
        let mut buf = [0; 1024];
        loop {
            let mut relay_queue: VecDeque<_> = vec![].into();
            let ready = self
                .socket_stream
                .ready(Interest::WRITABLE | Interest::READABLE)
                .await
                .expect("could not get ready state");

            match self.lsp_connection.receiver.try_recv() {
                Ok(msg) => {
                    relay_queue.push_back(msg);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    return Err(other_err!("disconnected from lsp client"))
                }
            }

            if let Some(msg) = relay_queue.pop_front() {
                if ready.is_writable() {
                    let v = serde_json::to_vec(&msg).expect("failed to serialize message");
                    match self.socket_stream.try_write(&v) {
                        Ok(_) => {
                            warn!("successfully relayed message");
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            warn!(
                                "writing from client stream would block, pushing msg back to queue"
                            );
                            relay_queue.push_front(msg);
                        }
                        Err(e) => {
                            panic!("error in client sending: {e:#?}")
                        }
                    }
                }
            }

            if ready.is_readable() {
                match self.socket_stream.try_read(&mut buf) {
                    Ok(0) => {
                        warn!("connection with server closed");
                        return Ok(());
                    }
                    Ok(n) => {
                        warn!("client read {n} bytes");
                        let msg = serde_json::from_slice::<lsp_server::Message>(&buf[..n])
                            .expect("failed to deserialize buffer");
                        warn!("Received on client side, relaying to lsp: {msg:#?}");
                        self.lsp_connection.sender.send(msg).unwrap();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        warn!("reading from server stream would block");
                    }
                    Err(e) => {
                        panic!("error in server receiving: {e:#?}")
                    }
                }
            }
        }
    }
}
