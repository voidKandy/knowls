use knowls::rpc::{LspMessage, LspRequest, Request, RpcMessage};
use lsp_server::RequestId;
use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use seraphic::{
    packet::{PacketRead, TcpPacket},
    RequestWrapper,
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpStream, ToSocketAddrs},
};

pub struct Server {
    should_exit: bool,
    app_connection: TcpStream,
    lsp_connection: lsp_server::Connection,
    /// we need to generate IDs for every `Notification` that gets relayed
    /// up to two chars are generated from this number
    lsp_id: u16,
    /// When this relays a shutdown request we need to know when to shutdown from a response from the application
    shutdown_request_id: Option<RequestId>,
}

impl Server {
    fn lsp_id_chars(&self) -> String {
        let mut str = String::new();
        // 97 is a 122 is z
        let first_letter = (((self.lsp_id / 25) + 97) as u8) as char;
        str.push(first_letter);
        if self.lsp_id > 25 {
            let second_letter = (((self.lsp_id % 25) + 97) as u8) as char;
            str.push(second_letter);
        }
        str
    }
    fn increment_lsp_id(&mut self) {
        if self.lsp_id == 625 {
            self.lsp_id = 0;
        } else {
            self.lsp_id += 1;
        }
    }

    pub async fn init(app_addr: impl ToSocketAddrs) -> super::MainResult<Self> {
        let app_connection = TcpStream::connect(app_addr).await?;
        Ok(Self {
            should_exit: false,
            lsp_id: 0,
            app_connection,
            lsp_connection: init_lsp_connection(),
            shutdown_request_id: None,
        })
    }

    pub async fn main_loop(&mut self) -> super::MainResult<()> {
        while !self.should_exit {
            match self.lsp_connection.receiver.try_recv() {
                Ok(msg) => {
                    tracing::warn!("received message from lsp: {msg:#?}");
                    self.handle_lsp_message(msg).await?;
                }
                Err(err) if err.is_empty() => {}
                Err(err) if err.is_disconnected() => {
                    tracing::error!("Lsp Disconnected");
                }
                Err(err) => {
                    tracing::error!("unexpected recv error: {err:#?}");
                }
            };
            // tracing::warn!("reading from app connection");
            if let Ok(result) = tokio::time::timeout(
                std::time::Duration::from_millis(200),
                TcpPacket::async_read(&mut self.app_connection),
            )
            .await
            {
                match result {
                    Ok(read) => match read {
                        PacketRead::Empty => {
                            tracing::warn!("returned empty");
                        }
                        PacketRead::Disconnected => {
                            tracing::warn!("disconnected from knowledge application");
                            break;
                        }
                        PacketRead::Message(msg) => {
                            tracing::warn!("received msg from application: {msg:#?}");
                            self.handle_application_message(msg).await?;
                        }
                    },
                    Err(e) => {
                        tracing::error!("problem reading from app connection: {e:#?}");
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_lsp_message(&mut self, msg: lsp_server::Message) -> super::MainResult<()> {
        let (id, msg) = match msg {
            lsp_server::Message::Request(ref req) => {
                if req.method.as_str() == "shutdown" {
                    tracing::warn!("received shutdown request: {req:#?}");
                    self.shutdown_request_id = Some(req.id.to_owned());
                }
                (req.id.to_string(), LspMessage::from(msg))
            }
            lsp_server::Message::Notification(ref _not) => {
                let r = (self.lsp_id_chars(), LspMessage::from(msg));
                self.increment_lsp_id();
                r
            }
            lsp_server::Message::Response(ref res) => (res.id.to_string(), LspMessage::from(msg)),
        };
        let req = Request::Lsp(LspRequest { msg });
        tracing::warn!("relaying: {req:#?}");
        TcpPacket::<RpcMessage>::async_write(&mut self.app_connection, &req.into_message(id))
            .await?;
        Ok(())
    }

    async fn handle_application_message(
        &mut self,
        msg: knowls::rpc::RpcMessage,
    ) -> super::MainResult<()> {
        match msg {
            knowls::rpc::RpcMessage::Res { id: _, res } => match res {
                knowls::rpc::Response::Lsp(lsp_message) => {
                    if let Some(shutdown_req_id) = self.shutdown_request_id.as_ref() {
                        tracing::warn!("verifying shutdown response: {lsp_message:#?}");
                        let lsp_msg: &lsp_server::Message = lsp_message.msg.as_ref();
                        if let lsp_server::Message::Response(lsp_server::Response { id, .. }) =
                            lsp_msg
                        {
                            if *id == *shutdown_req_id {
                                self.should_exit = false;
                            }
                        }
                    }
                    self.lsp_connection.sender.send(lsp_message.msg.into())?;
                }
                _ => {
                    tracing::warn!("no branch for {res:#?}");
                }
            },
            _ => {
                tracing::warn!("no branch for {msg:#?}");
            }
        }
        Ok(())
    }
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
            trigger_characters: None,
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
