use espx_lsp_server::{
    self,
    client::Client,
    config::Config,
    rpc::messages::{LspRelayRequest, Request, Response, RpcMessage, RpcPacket},
    server::Server,
    trace::CLI_TRACING,
    trace_panics,
};
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, ShowMessageParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use seraphic::packet::PacketRead;
use std::{collections::VecDeque, sync::LazyLock, time::Duration};
use tokio::{io::Interest, time::sleep};

struct RelayClient {
    lsp_send: crossbeam_channel::Sender<lsp_server::Message>,
    lsp_recv: crossbeam_channel::Receiver<lsp_server::Message>,
    lsp_message_queue: VecDeque<lsp_server::Message>,
    rpc_client: Client,
}

fn init_lsp_connection() -> lsp_server::Connection {
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
    let _: InitializeParams = serde_json::from_value(params).unwrap();
    lsp_connection
}

impl RelayClient {
    pub fn new(rpc_client: Client, lsp_connection: lsp_server::Connection) -> Self {
        Self {
            lsp_send: lsp_connection.sender,
            lsp_recv: lsp_connection.receiver,
            rpc_client,
            lsp_message_queue: VecDeque::new(),
        }
    }

    pub async fn main_loop(&mut self) {
        let mut id = 0;
        loop {
            if let Some(msg) = self.lsp_recv.try_recv().ok() {
                self.lsp_message_queue.push_front(msg);
            }

            let ready = self
                .rpc_client
                .stream
                .ready(Interest::WRITABLE | Interest::READABLE)
                .await
                .expect("failed to get ready state");

            if ready.is_readable() {
                match RpcPacket::async_read(&mut self.rpc_client.stream).await {
                    Ok(PacketRead::Message(msg)) => match msg {
                        RpcMessage::Res { res, .. } => {
                            if let Response::Lsp(relay_response) = res {
                                if let Some(res) = relay_response.payload {
                                    let lsp_response: lsp_server::Message = serde_json::from_value(res).expect("failed to serialize lsp relay response as lsp_server::Message");
                                    self.lsp_send.send(lsp_response).unwrap();
                                }
                            }
                        }
                        _ => {
                            panic!("relay received an unexpected message: {msg:#?}");
                        }
                    },
                    Ok(PacketRead::Disconnected) => {
                        tracing::warn!("relay received disconnect from server");
                        break;
                    }
                    Ok(PacketRead::Empty) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        panic!("failed to read from rpc client on client side: {e:#?}");
                    }
                }
            } else if ready.is_writable() && !self.lsp_message_queue.is_empty() {
                if let Some(msg) = self.lsp_message_queue.pop_front() {
                    let req = Request::from(LspRelayRequest::from(msg));

                    self.rpc_client
                        .send(req, &id.to_string())
                        .await
                        .expect("client failed to send");
                    id += 1;
                    tracing::warn!("updated id: {id}");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    tracing::warn!("spinning up headless Language Server");
    trace_panics!();

    let config = Config::init_from_global_config().expect("failed to build config");
    tracing::warn!("initializing with config: {config:#?}");

    let addr = "127.0.0.1:1598";
    let lsp_connection = init_lsp_connection();
    let mut client = match Client::connect(addr).await {
        Ok(client) => RelayClient::new(client, lsp_connection),
        Err(_) => {
            let params = ShowMessageParams {
                typ: lsp_types::MessageType::WARNING,
                message: String::from("ESPX Server is not running"),
            };
            lsp_connection
                .sender
                .send(lsp_server::Message::Notification(
                    lsp_server::Notification {
                        method: "window/showMessage".to_string(),
                        params: serde_json::to_value(params)
                            .expect("failed to serialize show message params"),
                    },
                ))
                .unwrap();
            return;
        }
    };

    client.main_loop().await
}
