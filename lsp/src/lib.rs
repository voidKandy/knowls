use lsp_types::{
    CodeActionProviderCapability, DiagnosticServerCapabilities, InitializeParams, ProgressParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkDoneProgressOptions,
};
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::warn;

async fn main_loop(
    mut tui_connection: UnixStream,
    unix_listener: UnixListener,
    lsp_connection: lsp_server::Connection,
    params: serde_json::Value,
) -> anyhow::Result<()> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    let (recv, sender) = (lsp_connection.receiver, lsp_connection.sender);

    tokio::spawn(async move {
        match unix_listener.accept().await {
            Ok((stream, _addr)) => {
                warn!("client: {_addr:?}");
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
            let json: serde_json::Value = serde_json::to_value(msg).unwrap();
            if let Some(str) = serde_json::to_string(&json).ok() {
                let str = &format!("{str}\n");
                let bytes: &[u8] = str.as_bytes();

                tui_connection.write_all(bytes).await.unwrap();
                tui_connection.flush().await.unwrap();
            }
        }
    });

    Ok(())
}

const TUI_SOCKET_ADDR: &str = "/tmp/espx_tui_socket.sock";
const LSP_SOCKET_ADDR: &str = "/tmp/espx_lsp_socket.sock";
#[tokio::main]
pub async fn start_lsp() -> anyhow::Result<()> {
    tracing::info!("starting LSP server");
    let (connection, io_threads) = lsp_server::Connection::stdio();

    if Path::new(LSP_SOCKET_ADDR).exists() {
        std::fs::remove_file(LSP_SOCKET_ADDR).unwrap();
    }
    let unix_listener = UnixListener::bind(LSP_SOCKET_ADDR).unwrap();
    warn!("created socket at: {LSP_SOCKET_ADDR}");
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let mut tui_connection = UnixStream::connect(TUI_SOCKET_ADDR).await?;
    tui_connection.write_all(b"hello world\n").await.unwrap();
    tui_connection.flush().await.unwrap();

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
    main_loop(
        tui_connection,
        unix_listener,
        connection,
        initialization_params,
    )
    .await?;
    io_threads.join()?;
    std::fs::remove_file(LSP_SOCKET_ADDR).unwrap();
    Ok(())
}
