use crossbeam_channel::{RecvError, TryRecvError};
use espx_lsp_server::{
    self,
    client::Client,
    config::Config,
    rpc::{messages::ServerRelayResponse, TcpPacket},
    server::Server,
    trace::CLI_TRACING,
    trace_panics, MainResult,
};
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use seraphic::RpcResponse;
use std::{collections::VecDeque, sync::LazyLock, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest, Ready},
    time::sleep,
};

struct HeadlessClient {
    lsp_send: crossbeam_channel::Sender<lsp_server::Message>,
    lsp_recv: crossbeam_channel::Receiver<lsp_server::Message>,
    lsp_message_queue: VecDeque<lsp_server::Message>,
    rpc_client: Client,
}

impl HeadlessClient {
    pub fn new(rpc_client: Client) -> Self {
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

        Self {
            lsp_send: lsp_connection.sender,
            lsp_recv: lsp_connection.receiver,
            rpc_client,
            lsp_message_queue: VecDeque::new(),
        }
    }

    pub async fn main_loop(&mut self) {
        let mut id = 0;
        let mut read_buffer = [0; 1024];
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
                match TcpPacket::read_from_stream(&mut self.rpc_client.stream).await {
                    Ok(msg) => {
                        let res: seraphic::socket::Response =
                            msg.try_into().expect("received request in client");
                        // let str = String::from_utf8_lossy(&read_buffer[..n]);
                        // tracing::warn!("client read {n} bytes, str: {str}");
                        // let msg: seraphic::socket::Response =
                        //     serde_json::from_slice(&read_buffer[..n])
                        //         .expect("failed to serialize relay response");

                        if let Ok(res) = ServerRelayResponse::try_from_response(&res)
                            .expect("failed to get from response")
                        {
                            if let Some(lsp_message) = res.payload {
                                self.lsp_send.send(lsp_message).unwrap();
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        panic!("failed to read from rpc client on client side: {e:#?}");
                    }
                }
            } else if ready.is_writable() && !self.lsp_message_queue.is_empty() {
                if let Some(msg) = self.lsp_message_queue.pop_front() {
                    let r = ServerRelayRequest::from(msg);
                    self.rpc_client
                        .send(r, &id.to_string())
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
    let mut client = match Client::connect(addr).await.map(HeadlessClient::new) {
        Ok(client) => client,
        Err(_) => {
            tracing::warn!("server is not running, need to spin up");
            let mut server = Server::new(config, addr).await;
            tracing::warn!("created server");
            tokio::spawn(async move { server.main_loop().await });
            sleep(Duration::from_millis(500)).await;
            Client::connect(addr)
                .await
                .map(HeadlessClient::new)
                .expect("should not fail to start relay client")
        }
    };

    client.main_loop().await
}
