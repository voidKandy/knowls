use seraphic::packet::{PacketRead, TcpPacket};
use tokio::net::{TcpStream, ToSocketAddrs};

use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams,
    ServerCapabilities, ShowMessageParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};

pub struct Server {
    app_connection: TcpStream,
    lsp_connection: lsp_server::Connection,
}

impl Server {
    pub async fn init(app_addr: impl ToSocketAddrs) -> super::MainResult<Self> {
        let app_connection = TcpStream::connect(app_addr).await?;
        Ok(Self {
            app_connection,
            lsp_connection: init_lsp_connection(),
        })
    }

    pub async fn main_loop(&mut self) -> super::MainResult<()> {
        loop {
            if let Ok(msg) = self.lsp_connection.receiver.recv() {
                tracing::warn!("recieved message from lsp: {msg:#?}");
                self.handle_lsp_message(msg).await?;
            }
            if let Ok(_) = self.app_connection.readable().await {
                match TcpPacket::async_read(&mut self.app_connection).await? {
                    PacketRead::Empty => {}
                    PacketRead::Disconnected => {
                        tracing::warn!("disconnected from knowledge application");
                        break;
                    }
                    PacketRead::Message(msg) => {
                        self.handle_application_message(msg).await?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_lsp_message(&mut self, msg: lsp_server::Message) -> super::MainResult<()> {
        Ok(())
    }
    async fn handle_application_message(
        &mut self,
        msg: knowls::rpc::RpcMessage,
    ) -> super::MainResult<()> {
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
