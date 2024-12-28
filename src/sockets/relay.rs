use crate::{
    handle::{self, buffer_operations::BufferOpChannelStatus},
    sockets::{init_clientside_listener_and_stream, CLIENTSIDE_RELAY_ADDR, SERVERSIDE_RELAY_ADDR},
    state::SharedState,
    MainResult,
};
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::RwLock,
};
use tracing::warn;

use super::SocketMessage;

impl SocketMessage for lsp_server::Message {}

/// Loop for handling any messages received by the relay on the client side
pub async fn from_relay_recv_loop(
    shared_stream: Arc<RwLock<UnixStream>>,
    listener: UnixListener,
    state: SharedState<'static>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                warn!("relay connected: {addr:?}");
                let unix_stream = Arc::clone(&shared_stream);
                state.0.try_write().unwrap().attached = Some(addr);

                let thread_running = Arc::new(RwLock::new(true));
                let shared_message_queue = Arc::new(RwLock::new(vec![]));
                let thread_message_queue = Arc::clone(&shared_message_queue);

                let thread_running_clone = Arc::clone(&thread_running);
                tokio::spawn(async move {
                    let mut buf_reader = BufReader::new(stream);
                    let mut buf = String::new();
                    loop {
                        let bytes = buf_reader.read_line(&mut buf).await.unwrap();
                        if bytes == 0 {
                            warn!("Closed");
                            *thread_running_clone.write().await = true;
                            break;
                        }

                        if let Some(msg) = serde_json::from_str::<lsp_server::Message>(&buf).ok() {
                            warn!("Rcv: {msg:#?}");
                            thread_message_queue.write().await.push(msg);
                        }
                        buf.clear();
                    }
                });

                loop {
                    if let Some(next_msg) = shared_message_queue.write().await.pop() {
                        match match next_msg {
                            lsp_server::Message::Notification(not) => {
                                handle::notifications::handle_notification(not, state.clone()).await
                            }
                            lsp_server::Message::Request(req) => {
                                if req.method.as_str() == "shutdown" {
                                    warn!("shutting down server");
                                    return;
                                }
                                handle::requests::handle_request(req, state.clone()).await
                            }
                            _ => Err(std::io::Error::other("No handler for responses").into()),
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
                    } else if !*thread_running.read().await {
                        warn!("relay receive thread stopped");
                        state.0.try_write().unwrap().attached = None;
                        break;
                    }
                }
            }
            Err(err) => {
                warn!("error connecting {err:#?}")
            }
        }
    }
}

async fn relay_main_loop(
    mut server_connection: UnixStream,
    unix_listener: UnixListener,
    lsp_connection: lsp_server::Connection,
    params: serde_json::Value,
) -> MainResult<()> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    let (recv, sender) = (lsp_connection.receiver, lsp_connection.sender);

    tokio::spawn(async move {
        match unix_listener.accept().await {
            Ok((stream, _addr)) => {
                warn!("espx server connected: {_addr:?}");
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
                            sender.send(msg).unwrap();
                        }
                        buf.clear();
                    }
                });
            }
            Err(err) => {
                println!("error connecting {err:#?}")
            }
        }
    });

    tokio::spawn(async move {
        for msg in &recv {
            if let Some(bytes) = msg.as_bytes_to_send().ok() {
                server_connection.write_all(&bytes).await.unwrap();
                server_connection.flush().await.unwrap();
            }
        }
    });

    Ok(())
}

pub async fn start_lsp_relay() -> MainResult<()> {
    tracing::info!("starting LSP RPC relay");
    let (connection, io_threads) = lsp_server::Connection::stdio();

    let (unix_listener, server_connection) =
        init_clientside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;

    let text_document_sync = Some(TextDocumentSyncCapability::Options(
        TextDocumentSyncOptions {
            open_close: Some(true),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(
                lsp_types::SaveOptions {
                    include_text: Some(true),
                },
            )),
            change: Some(TextDocumentSyncKind::INCREMENTAL),

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

    let initialization_params = connection.initialize(server_capabilities)?;
    relay_main_loop(
        server_connection,
        unix_listener,
        connection,
        initialization_params,
    )
    .await?;
    io_threads.join()?;
    std::fs::remove_file(CLIENTSIDE_RELAY_ADDR).unwrap();
    Ok(())
}
